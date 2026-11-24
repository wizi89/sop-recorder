use std::time::Duration;

use super::auth;
use super::sse::SSEResultPayload;

/// Poll the server for a generation job's result.
/// Returns the result once the job completes, or an error if it fails/times out.
///
/// If a poll attempt gets a 401, the token is refreshed via `api_base` before
/// retrying.  This prevents the token from expiring during long poll windows.
///
/// A request that fails on the network, a 5xx or an unreadable body counts as
/// one failed attempt; only a rejected session or a failed job ends the poll
/// early. With 40 attempts the backoff alone adds up to about 19 minutes, plus
/// up to 15 seconds per attempt that hangs until the client timeout.
pub async fn poll_job_result(
    initial_token: &str,
    job_id: &str,
    api_url: Option<&str>,
    api_base: &str,
    max_attempts: u32,
) -> Result<SSEResultPayload, String> {
    poll_with_backoff(
        initial_token,
        job_id,
        api_url,
        api_base,
        max_attempts,
        backoff_delay,
    )
    .await
}

/// Exponential backoff: 3s, 6s, 12s, ... capped at 30s.
fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_secs(std::cmp::min(3 * (1u64 << attempt.min(4)), 30))
}

/// reqwest's own message is only "error sending request for url (...)"; the
/// reason (refused, reset, timed out, DNS, TLS) sits in the source chain.
fn with_causes(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

async fn poll_with_backoff(
    initial_token: &str,
    job_id: &str,
    api_url: Option<&str>,
    api_base: &str,
    max_attempts: u32,
    backoff: fn(u32) -> Duration,
) -> Result<SSEResultPayload, String> {
    let base_url = api_url.unwrap_or_else(|| super::auth::api_url_for_target(None));
    let url = format!("{}/generate/{}/result", base_url, job_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut token = initial_token.to_string();
    let mut last_error = String::from("no attempt was made");

    for attempt in 0..max_attempts {
        let delay = backoff(attempt);
        tokio::time::sleep(delay).await;

        // Every attempt leaves a trace. The 2026-09-03 test saw this recovery
        // twice and the server side showed nothing, which left no way to tell
        // a slow generation from a flapping connection.
        log::info!(
            "Poll attempt {} of {} for job {} (waited {}s)",
            attempt + 1,
            max_attempts,
            job_id,
            delay.as_secs(),
        );

        // A request that never gets an answer is retried like a 5xx. Polling
        // is the recovery from a dropped SSE stream, so the network is often
        // still flapping here: giving up on the first failure lost a result
        // the server had finished minutes earlier.
        let response = match client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                let cause = with_causes(&e);
                log::warn!(
                    "Poll attempt {} failed to reach the server: {}",
                    attempt + 1,
                    cause
                );
                last_error = format!("Poll failed: {}", cause);
                continue;
            }
        };

        // Token expired during polling -- refresh and retry this attempt
        if response.status().as_u16() == 401 {
            log::warn!("Poll attempt {} -- token expired, refreshing", attempt + 1);
            match auth::refresh_session(api_base).await {
                Ok(Some(session)) => {
                    token = session.access_token;
                    last_error = String::from("token refreshed after a 401");
                    continue;
                }
                // Only a rejected refresh token means the session is over.
                Ok(None) => {
                    return Err("Sitzung abgelaufen. Bitte erneut anmelden.".into());
                }
                // A refresh that failed on the network is as transient as
                // the poll itself: try again on the next attempt.
                Err(e) => {
                    log::warn!("Poll attempt {} -- {}", attempt + 1, e);
                    last_error = e;
                    continue;
                }
            }
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            log::warn!(
                "Poll attempt {} -- server returned {} {}",
                attempt + 1,
                status,
                body
            );
            last_error = format!("server returned {}", status);
            continue;
        }

        // A body cut off mid-transfer, or a proxy's HTML page, is as
        // transient as a failed request.
        let body: serde_json::Value = match response.json().await {
            Ok(body) => body,
            Err(e) => {
                log::warn!("Poll attempt {} -- unreadable body: {}", attempt + 1, e);
                last_error = format!("Poll parse error: {}", with_causes(&e));
                continue;
            }
        };

        let status = body.get("status").and_then(|v| v.as_str()).unwrap_or("");

        match status {
            "completed" => {
                log::info!("Poll: job {} completed on attempt {}", job_id, attempt + 1);
                let enriched = body
                    .get("enriched")
                    .cloned()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();
                let markdown = body
                    .get("markdown")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let job_id = body
                    .get("job_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let pdf_url = body
                    .get("pdf_url")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                return Ok(SSEResultPayload {
                    enriched,
                    markdown,
                    job_id,
                    pdf_url,
                });
            }
            "processing" => {
                log::info!(
                    "Poll: job {} still processing (attempt {})",
                    job_id,
                    attempt + 1
                );
                last_error = String::from("job still processing");
            }
            "failed" | "partial" => {
                let error = body
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Generation failed on server");
                return Err(error.to_string());
            }
            _ => {
                log::warn!("Poll: unexpected status '{}' for job {}", status, job_id);
                last_error = format!("unexpected status '{}'", status);
            }
        }
    }

    Err(format!(
        "Generation job {} did not complete after {} poll attempts (last: {})",
        job_id, max_attempts, last_error
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    /// What the test server does with one incoming connection.
    enum Reply {
        /// Read the request, then close the socket without answering: the
        /// client sees a transport error, as on a flapping network.
        Drop,
        /// Answer 200 with this body.
        Ok(&'static str),
    }

    /// A tiny HTTP server that handles one connection per scripted reply, in
    /// order. A mock crate would be a new dependency for a handful of tests.
    fn serve(replies: Vec<Reply>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }
                match reply {
                    Reply::Drop => drop(stream),
                    Reply::Ok(body) => {
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        stream.write_all(response.as_bytes()).unwrap();
                        stream.flush().unwrap();
                    }
                }
            }
        });
        format!("http://{}", addr)
    }

    /// An address nothing listens on: every request is refused.
    fn refused_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        format!("http://{}", addr)
    }

    fn no_wait(_attempt: u32) -> Duration {
        Duration::ZERO
    }

    const COMPLETED: &str =
        r##"{"status":"completed","markdown":"# Anleitung","job_id":"job-1","enriched":[]}"##;

    async fn poll(url: &str, max_attempts: u32) -> Result<SSEResultPayload, String> {
        poll_with_backoff("token", "job-1", Some(url), url, max_attempts, no_wait).await
    }

    #[tokio::test]
    async fn a_dropped_connection_does_not_end_the_poll() {
        let url = serve(vec![Reply::Drop, Reply::Ok(COMPLETED)]);

        let result = poll(&url, 5).await.expect("the second attempt completes");

        assert_eq!(result.markdown, "# Anleitung");
        assert_eq!(result.job_id.as_deref(), Some("job-1"));
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_does_not_end_the_poll() {
        let url = serve(vec![
            Reply::Ok("<html>Gateway</html>"),
            Reply::Ok(COMPLETED),
        ]);

        let result = poll(&url, 5).await.expect("the second attempt completes");

        assert_eq!(result.markdown, "# Anleitung");
    }

    #[tokio::test]
    async fn an_unreachable_server_fails_after_every_attempt_naming_the_last_error() {
        let url = refused_url();

        let error = poll(&url, 2).await.expect_err("nothing ever answers");

        assert!(error.contains("job-1"), "{error}");
        assert!(error.contains("2 poll attempts"), "{error}");
        assert!(error.contains("error sending request"), "{error}");
        // The reason from the source chain, not only reqwest's summary.
        assert!(error.contains("os error"), "{error}");
    }

    #[test]
    fn backoff_doubles_from_three_seconds_and_stops_at_thirty() {
        let secs: Vec<u64> = (0..7).map(|a| backoff_delay(a).as_secs()).collect();
        assert_eq!(secs, vec![3, 6, 12, 24, 30, 30, 30]);
    }
}
