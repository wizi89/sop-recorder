//! Submitting an error report to the recorder's own server.
//!
//! Design D2: the report goes to `POST /client-reports` on the server the
//! recorder already talks to, with the bearer token it already holds, and the
//! server hands it to its own error tracker. No tracker DSN ships in this
//! binary, so nobody can post to our tracker with a key lifted out of the
//! app, and a self-hosted installation -- which overrides the API URL -- sends
//! its reports to its own server by construction.
//!
//! The server answers with the report number even when its tracker is down
//! (D9), so a 2xx here always means "handled, stop retrying".

use std::time::Duration;

use crate::error_reports::ErrorReport;

use super::auth;

/// Why a submission did not succeed. The distinction matters to the caller:
/// the file stays on disk either way (D7), but only `Unauthorized` is worth
/// resolving by refreshing the session first.
#[derive(Debug)]
pub enum SubmitError {
    Unauthorized,
    Failed(String),
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubmitError::Unauthorized => write!(f, "Sitzung abgelaufen"),
            SubmitError::Failed(msg) => write!(f, "{}", msg),
        }
    }
}

/// The body the server validates. Built field by field rather than by
/// serialising the report, so the local-only `consent` bookkeeping cannot
/// leak into a request and the wire shape stays visible in one place.
fn body(report: &ErrorReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": report.schema_version,
        "report_id": report.report_id,
        "kind": report.kind.as_str(),
        "occurred_at": report.occurred_at,
        "app_version": report.app_version,
        "os": report.os,
        "os_version": report.os_version,
        "arch": report.arch,
        "locale": report.locale,
        "phase": report.phase.as_str(),
        "message": report.message,
        "location": report.location,
        "log_tail": report.log_tail,
        "settings": report.settings,
        "job_id": report.job_id,
        "comment": report.comment,
    })
}

/// Post one report. Returns the report number the server answered with.
///
/// One 401 is resolved by refreshing the session and retrying, following the
/// pattern in `jobs.rs`: a report can sit on disk for days, so its token being
/// stale on the way out is the ordinary case rather than the exception.
pub async fn submit_report(
    report: &ErrorReport,
    api_base: &str,
    access_token: &str,
) -> Result<String, SubmitError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| SubmitError::Failed(format!("HTTP client error: {}", e)))?;
    let url = format!("{}/client-reports", api_base);
    let payload = body(report);

    let mut token = access_token.to_string();
    let mut refreshed = false;

    loop {
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| SubmitError::Failed(format!("Fehlerbericht konnte nicht gesendet werden: {}", e)))?;

        let status = response.status();
        if status.as_u16() == 401 && !refreshed {
            refreshed = true;
            match auth::refresh_session(api_base).await {
                Ok(Some(session)) => {
                    token = session.access_token;
                    continue;
                }
                _ => return Err(SubmitError::Unauthorized),
            }
        }
        if status.as_u16() == 401 {
            return Err(SubmitError::Unauthorized);
        }
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            log::warn!("Fehlerbericht abgelehnt: {} - {}", status, text);
            return Err(SubmitError::Failed(format!("Server hat mit {} geantwortet", status)));
        }

        let parsed: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SubmitError::Failed(format!("Antwort nicht lesbar: {}", e)))?;
        // The server is authoritative about the number, but a server that
        // answered 200 has handled the report either way, so a missing field
        // must not turn a success into a retry.
        return Ok(parsed
            .get("report_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| report.number()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_reports::{ErrorReport, Phase, ReportKind};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// A one-request HTTP server. Enough to answer a single POST with a fixed
    /// status and body, which is all this client needs proving against; a mock
    /// crate would be a new dependency for two tests.
    fn serve_once(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut request_body = vec![0u8; content_length];
            reader.read_exact(&mut request_body).unwrap();
            let response = format!(
                "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        });
        format!("http://{}", addr)
    }

    fn report() -> ErrorReport {
        ErrorReport::new(
            ReportKind::CommandError,
            Phase::Processing,
            "Upload failed: 500".to_string(),
            None,
            vec!["[INFO] Upload gestartet".to_string()],
        )
    }

    #[tokio::test]
    async fn a_success_returns_the_servers_report_number() {
        let base = serve_once("HTTP/1.1 200 OK", r#"{"report_id":"a1b2c3d4"}"#);
        let number = submit_report(&report(), &base, "token").await.unwrap();
        assert_eq!(number, "a1b2c3d4");
    }

    #[tokio::test]
    async fn a_server_outage_is_a_failure_so_the_caller_keeps_the_file() {
        let base = serve_once("HTTP/1.1 503 Service Unavailable", r#"{"detail":"down"}"#);
        let err = submit_report(&report(), &base, "token").await.unwrap_err();
        assert!(matches!(err, SubmitError::Failed(_)), "got {err:?}");
    }

    #[test]
    fn the_request_body_carries_no_consent_field() {
        let mut r = report();
        r.consent = crate::error_reports::Consent::Granted;
        let payload = body(&r);
        assert!(payload.get("consent").is_none());
        assert_eq!(payload["kind"], "command_error");
        assert_eq!(payload["phase"], "processing");
    }
}
