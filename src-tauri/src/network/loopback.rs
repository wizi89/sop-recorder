//! The loopback callback listener. **This is the deep-link swap point.**
//!
//! OpenSpec change: browser-login. When the app registers the `cogniclone://`
//! URL scheme, this entire file is deleted and replaced by a
//! `start_deep_link()` returning the same `Callback` shape, fed by
//! `tauri-plugin-deep-link`'s `on_open_url`, with a `Reply` whose `finish`
//! does nothing -- under a scheme the browser has no connection waiting on the
//! app. Nothing in `network/auth.rs`, the command, the server or the UI
//! changes. See `openspec/changes/browser-login/tasks.md`, task 8.2.
//!
//! It is ~200 lines over `tokio::net::TcpListener` rather than a web
//! framework: its whole job is to parse one request line, in code scheduled
//! for deletion. Anything it cannot parse is answered 400 and ignored.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;
use url::Url;

/// How long one connection has to send its request line before it is dropped.
const READ_DEADLINE: Duration = Duration::from_secs(5);

/// How long `Reply` holds the browser's connection waiting for an outcome.
///
/// The agreed ordering is strict at every step: server upstream bound (10) <
/// recorder exchange deadline (15) < this (20). Equal values are a bug -- an
/// exchange that succeeds in the last milliseconds would lose to a timer that
/// started at the same moment, and the tab would report failure while the app
/// signed in.
const FINISH_BOUND: Duration = Duration::from_secs(20);

/// What the browser is told, as a fixed token. Never a server message: nothing
/// user- or token-derived may land in browser history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    /// The `state` did not match, or the callback was malformed.
    Verification,
    /// The user declined on the consent screen.
    Denied,
    /// The server does not support browser sign-in.
    Unsupported,
    /// Anything else, including a keyring failure and a cancelled attempt.
    Error,
}

impl Outcome {
    fn reason(self) -> Option<&'static str> {
        match self {
            Outcome::Success => None,
            Outcome::Verification => Some("verification"),
            Outcome::Denied => Some("denied"),
            Outcome::Unsupported => Some("unsupported"),
            Outcome::Error => Some("error"),
        }
    }

    fn location(self, webapp: &str) -> String {
        let webapp = webapp.trim_end_matches('/');
        match self.reason() {
            None => format!("{}/desktop/signed-in", webapp),
            Some(reason) => format!("{}/desktop/sign-in-failed?reason={}", webapp, reason),
        }
    }
}

/// The browser's connection, held open until the recorder knows the outcome.
///
/// Dropping it without calling `finish` answers the failure redirect, so the
/// browser never hangs on the connection -- a cancel or a panic included.
pub struct Reply {
    stream: Option<TcpStream>,
    webapp: String,
}

impl Reply {
    /// Answer the browser with a 302 to the web app's result page.
    pub async fn finish(mut self, outcome: &Outcome) {
        if let Some(mut stream) = self.stream.take() {
            let location = outcome.location(&self.webapp);
            // Bounded: if the browser has gone away, this must not hold the
            // command open.
            let _ =
                tokio::time::timeout(FINISH_BOUND, write_redirect(&mut stream, &location)).await;
        }
    }

    /// Test seam: the bound shortened so the timeout is observable.
    #[cfg(test)]
    async fn finish_with_bound(mut self, outcome: &Outcome, bound: Duration) {
        if let Some(mut stream) = self.stream.take() {
            let location = outcome.location(&self.webapp);
            let _ = tokio::time::timeout(bound, write_redirect(&mut stream, &location)).await;
        }
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            let location = Outcome::Error.location(&self.webapp);
            // Best effort, from a context that cannot await: the browser gets
            // the failure redirect rather than a dangling connection.
            tokio::spawn(async move {
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    write_redirect(&mut stream, &location),
                )
                .await;
            });
        }
    }
}

async fn write_redirect(stream: &mut TcpStream, location: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        location
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

async fn write_bad_request(stream: &mut TcpStream) {
    let _ = stream
        .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .await;
    let _ = stream.flush().await;
}

/// A bound listener waiting for the browser's callback.
pub struct Callback {
    /// What to send as `redirect_uri`. The only mechanism-specific value the
    /// command ever sees.
    pub redirect_uri: String,
    listener: TcpListener,
    webapp: String,
}

/// Bind `127.0.0.1:0` and let the OS choose the port.
///
/// Loopback rather than `0.0.0.0`, so the socket is not reachable from the
/// network; port 0 rather than a fixed port, so there is nothing to collide
/// with and nothing to probe.
pub async fn start_loopback(webapp: &str) -> Result<Callback, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("Could not open the local callback port: {}", e))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    Ok(Callback {
        redirect_uri: format!("http://127.0.0.1:{}/callback", port),
        listener,
        webapp: webapp.to_string(),
    })
}

impl Callback {
    /// The bound address, for tests and for the attempt log line.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Wait for a well-formed `/callback`, the timeout, or a cancel.
    ///
    /// **Accepts in a loop.** A single `accept()` would be smaller and is
    /// wrong: browsers routinely open speculative and preconnect TCP
    /// connections that carry no request bytes, and any local process can
    /// connect to the port. Under one-shot accept such a connection consumes
    /// the attempt and -- with no read deadline -- blocks on a read while the
    /// real callback lands on a connection nobody accepts. The user then waits
    /// out the full five minutes, intermittently and unreproducibly.
    pub async fn recv(
        self,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<(Url, Reply), String> {
        let Callback {
            listener, webapp, ..
        } = self;

        // Each connection is read in its own task, and the first one to
        // produce a well-formed callback wins. Reading them one after another
        // would be smaller and is wrong: a connection that says nothing holds
        // the loop for the whole read deadline, which is the preconnect case
        // at a narrower width rather than a fixed one (task 4.5).
        let (found_tx, mut found_rx) = tokio::sync::mpsc::channel::<(Url, TcpStream)>(4);

        let accept_loop = async {
            loop {
                let (mut stream, _peer) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(e) => {
                        log::warn!("Callback listener accept failed: {}", e);
                        continue;
                    }
                };

                let found_tx = found_tx.clone();
                tokio::spawn(async move {
                    match read_request(&mut stream).await {
                        Request::Callback(url) => {
                            // If the attempt has already resolved, the receiver
                            // is gone and the stream is dropped here, which
                            // closes the connection rather than leaking it.
                            let _ = found_tx.send((url, stream)).await;
                        }
                        Request::Unusable => {
                            // A request line arrived, but it is not our
                            // callback: another path, or not a GET. Answered,
                            // then dropped; the attempt stays live.
                            write_bad_request(&mut stream).await;
                        }
                        Request::Nothing => {
                            // Nothing before the deadline: a browser preconnect
                            // or a local probe. Dropped silently.
                            drop(stream);
                        }
                    }
                });
            }
        };

        tokio::select! {
            Some((url, stream)) = found_rx.recv() => Ok((
                url,
                Reply {
                    stream: Some(stream),
                    webapp: webapp.clone(),
                },
            )),
            // Never completes; it is here so that accepting continues while
            // the branches above wait.
            _ = accept_loop => unreachable!("the accept loop does not return"),
            _ = tokio::time::sleep(timeout) => {
                Err("Die Anmeldung im Browser wurde nicht abgeschlossen.".to_string())
            }
            _ = cancel.cancelled() => {
                Err("Die Anmeldung wurde abgebrochen.".to_string())
            }
        }
        // Dropping `listener` here releases the port on every exit, including
        // the timeout and the cancel.
    }
}

/// What one connection turned out to be.
enum Request {
    /// A GET of our callback path.
    Callback(Url),
    /// A request line arrived, but it is not one we answer: another path, or
    /// not a GET. Gets a 400 -- it is a real client waiting for an answer.
    Unusable,
    /// Nothing arrived before the deadline, or the peer closed without
    /// sending. Gets nothing: there is no one to answer.
    Nothing,
}

async fn read_request(stream: &mut TcpStream) -> Request {
    match read_request_line(stream).await {
        None => Request::Nothing,
        Some(line) => match callback_from_request_line(&line) {
            Some(url) => Request::Callback(url),
            None => Request::Unusable,
        },
    }
}

/// Read the request line, or None if nothing arrived before the deadline.
async fn read_request_line(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];

    let read = async {
        loop {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None; // closed without sending a request
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
                return String::from_utf8(buf[..pos].to_vec()).ok();
            }
            if buf.len() > 8192 {
                return None; // not a request line we are going to like
            }
        }
    };

    tokio::time::timeout(READ_DEADLINE, read).await.ok()?
}

/// "GET /callback?code=..&state=.. HTTP/1.1" -> the callback URL.
fn callback_from_request_line(line: &str) -> Option<Url> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    if method != "GET" {
        return None;
    }
    parse_target(target)
}

/// Resolve the request target against the loopback origin and keep it only if
/// it is our callback path.
fn parse_target(target: &str) -> Option<Url> {
    // The base is arbitrary but fixed: the command cares about the query, and
    // `parse_callback` is origin-agnostic by design.
    let url = Url::options()
        .base_url(Some(&Url::parse("http://127.0.0.1/").unwrap()))
        .parse(target)
        .ok()?;
    if url.path() != "/callback" {
        return None;
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEBAPP: &str = "https://app.example.test";

    async fn read_response(stream: &mut TcpStream) -> String {
        let mut buf = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut buf)).await;
        String::from_utf8_lossy(&buf).to_string()
    }

    // -- 4.2 ----------------------------------------------------------------

    #[tokio::test]
    async fn binds_loopback_on_an_os_chosen_port() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_ne!(addr.port(), 0);
        assert_eq!(
            cb.redirect_uri,
            format!("http://127.0.0.1:{}/callback", addr.port())
        );
    }

    // -- 4.1 ----------------------------------------------------------------

    #[tokio::test]
    async fn yields_the_callback_url_and_holds_the_connection_open() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();

        let task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /callback?code=X&state=Y HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();

        let (url, reply) = task.await.unwrap().unwrap();
        let (code, state) = crate::network::auth::parse_callback(&url).unwrap();
        assert_eq!(code, "X");
        assert_eq!(state, "Y");

        // Nothing has been answered yet: the outcome is not known.
        let mut peek = [0u8; 1];
        let got = tokio::time::timeout(Duration::from_millis(300), client.read(&mut peek)).await;
        assert!(
            got.is_err(),
            "the browser was answered before the outcome was known"
        );

        reply.finish(&Outcome::Success).await;
        let response = read_response(&mut client).await;
        assert!(response.contains("302"), "{}", response);
    }

    // -- 4.1b ---------------------------------------------------------------

    async fn outcome_response(outcome: Outcome) -> String {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /callback?code=CODEVALUE&state=STATEVALUE HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let (_url, reply) = task.await.unwrap().unwrap();
        reply.finish(&outcome).await;
        read_response(&mut client).await
    }

    #[tokio::test]
    async fn success_redirects_to_the_signed_in_page() {
        let response = outcome_response(Outcome::Success).await;
        assert!(response.contains("302 Found"), "{}", response);
        assert!(
            response.contains("Location: https://app.example.test/desktop/signed-in"),
            "{}",
            response
        );
    }

    #[tokio::test]
    async fn each_failure_carries_its_own_reason_token() {
        for (outcome, token) in [
            (Outcome::Verification, "verification"),
            (Outcome::Denied, "denied"),
            (Outcome::Unsupported, "unsupported"),
            (Outcome::Error, "error"),
        ] {
            let response = outcome_response(outcome).await;
            assert!(
                response.contains(&format!("sign-in-failed?reason={}", token)),
                "{:?} -> {}",
                outcome,
                response
            );
        }
    }

    #[tokio::test]
    async fn no_response_leaks_the_code_the_state_or_an_email() {
        for outcome in [Outcome::Success, Outcome::Verification, Outcome::Error] {
            let response = outcome_response(outcome).await;
            assert!(!response.contains("CODEVALUE"), "{}", response);
            assert!(!response.contains("STATEVALUE"), "{}", response);
            assert!(!response.contains('@'), "{}", response);
        }
    }

    #[tokio::test]
    async fn a_dropped_reply_still_answers_the_browser() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /callback?code=X&state=Y HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let (_url, reply) = task.await.unwrap().unwrap();

        drop(reply);
        let response = read_response(&mut client).await;
        assert!(
            response.contains("sign-in-failed?reason=error"),
            "{}",
            response
        );
    }

    // -- 4.1c ---------------------------------------------------------------

    #[tokio::test]
    async fn an_unanswered_reply_releases_the_browser_at_its_bound() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /callback?code=X&state=Y HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let (_url, reply) = task.await.unwrap().unwrap();

        // The real bound is 20s; shortened here so the behaviour is observable.
        let started = tokio::time::Instant::now();
        reply
            .finish_with_bound(&Outcome::Error, Duration::from_millis(100))
            .await;
        assert!(started.elapsed() < Duration::from_secs(2));

        let response = read_response(&mut client).await;
        assert!(response.contains("reason=error"), "{}", response);
    }

    #[test]
    fn the_bounds_are_strictly_ordered() {
        // server upstream (10) < recorder exchange deadline (15) < finish (20).
        assert!(Duration::from_secs(10) < Duration::from_secs(15));
        assert!(Duration::from_secs(15) < FINISH_BOUND);
    }

    // -- 4.3 ----------------------------------------------------------------

    #[tokio::test]
    async fn a_malformed_request_is_answered_400_and_does_not_resolve_recv() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let mut task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(b"GARBAGE\r\n\r\n").await.unwrap();
        let response = read_response(&mut client).await;
        assert!(response.contains("400"), "{}", response);

        // Still pending: the attempt was not consumed.
        let settled = tokio::time::timeout(Duration::from_millis(300), &mut task).await;
        assert!(settled.is_err(), "a malformed request resolved the attempt");
        task.abort();
    }

    #[tokio::test]
    async fn another_path_does_not_resolve_recv() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let mut task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

        let mut client = TcpStream::connect(addr).await.unwrap();
        client
            .write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        assert!(read_response(&mut client).await.contains("400"));

        let settled = tokio::time::timeout(Duration::from_millis(300), &mut task).await;
        assert!(settled.is_err());
        task.abort();
    }

    // -- 4.4 ----------------------------------------------------------------

    #[tokio::test]
    async fn a_timeout_returns_an_error_and_releases_the_port() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();

        let result = cb.recv(Duration::from_millis(50), &cancel).await;
        assert!(result.is_err());

        // The port is free again: rebinding it proves the listener was dropped.
        let rebound = TcpListener::bind(addr).await;
        assert!(
            rebound.is_ok(),
            "the port was still bound after the timeout"
        );
    }

    #[tokio::test]
    async fn a_cancel_while_waiting_drops_the_listener_at_once() {
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();

        let token = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            token.cancel();
        });

        let started = tokio::time::Instant::now();
        let result = cb.recv(Duration::from_secs(300), &cancel).await;
        assert!(result.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "cancel did not take effect"
        );
        assert!(TcpListener::bind(addr).await.is_ok());
    }

    // -- 4.5 ----------------------------------------------------------------

    #[tokio::test]
    async fn a_silent_connection_cannot_strand_the_attempt() {
        // The browser-preconnect case. A one-shot accept() fails this test.
        let cb = start_loopback(WEBAPP).await.unwrap();
        let addr = cb.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move { cb.recv(Duration::from_secs(10), &cancel).await });

        // Connects and says nothing. Held open for the whole test.
        let _silent = TcpStream::connect(addr).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut real = TcpStream::connect(addr).await.unwrap();
        real.write_all(b"GET /callback?code=X&state=Y HTTP/1.1\r\n\r\n")
            .await
            .unwrap();

        // Resolves from the second connection without waiting out the first,
        // and well inside the 5-second read deadline.
        let started = tokio::time::Instant::now();
        let (url, _reply) = tokio::time::timeout(Duration::from_secs(4), task)
            .await
            .expect("recv did not resolve: the silent connection stranded the attempt")
            .unwrap()
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(4));
        assert_eq!(
            crate::network::auth::parse_callback(&url).unwrap(),
            ("X".to_string(), "Y".to_string())
        );
    }

    // -- target parsing -----------------------------------------------------

    #[test]
    fn only_the_callback_path_is_accepted() {
        assert!(parse_target("/callback?code=a&state=b").is_some());
        assert!(parse_target("/callback").is_some());
        assert!(parse_target("/other").is_none());
        assert!(parse_target("/").is_none());
    }

    #[test]
    fn an_absolute_form_target_is_accepted() {
        // Some clients send the absolute form in the request line.
        assert!(parse_target("http://127.0.0.1:1234/callback?code=a&state=b").is_some());
    }
}
