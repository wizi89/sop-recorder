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

/// Regression: the browser must be able to keep the answer.
///
/// Only the request line is parsed, so the headers stay unread. Closing on
/// them resets the connection, and a browser throws away the response it
/// has already read and retries -- onto a listener that is gone by then.
#[tokio::test]
async fn the_rest_of_the_request_is_read_so_the_close_is_not_a_reset() {
    let cb = start_loopback(WEBAPP).await.unwrap();
    let addr = cb.local_addr().unwrap();
    let cancel = CancellationToken::new();
    let task = tokio::spawn(async move { cb.recv(Duration::from_secs(5), &cancel).await });

    let mut client = TcpStream::connect(addr).await.unwrap();
    let mut request =
        b"GET /callback?code=X&state=Y HTTP/1.1\r\nHost: 127.0.0.1\r\nCookie: ".to_vec();
    // Well past anything the parser touches, so a close on it is a reset
    // rather than a buffer that happened to be empty.
    request.extend_from_slice(&vec![b'x'; 16 * 1024]);
    request.extend_from_slice(b"\r\n\r\n");
    client.write_all(&request).await.unwrap();

    let (_url, reply) = task.await.unwrap().unwrap();
    reply.finish(&Outcome::Success).await;

    let mut response = Vec::new();
    client
        .read_to_end(&mut response)
        .await
        .expect("the connection was reset instead of closed");
    assert!(
        String::from_utf8_lossy(&response).contains("302"),
        "{:?}",
        String::from_utf8_lossy(&response)
    );
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
