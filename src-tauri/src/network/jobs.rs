use std::time::Duration;

use crate::config;
use super::auth;
use super::sse::SSEResultPayload;

/// Poll the server for a generation job's result.
/// Returns the result once the job completes, or an error if it fails/times out.
///
/// If a poll attempt gets a 401, the token is refreshed via `api_base` before
/// retrying.  This prevents the token from expiring during long poll windows
/// (~10 minutes).
pub async fn poll_job_result(
    initial_token: &str,
    job_id: &str,
    api_url: Option<&str>,
    api_base: &str,
    max_attempts: u32,
) -> Result<SSEResultPayload, String> {
    let base_url = api_url.unwrap_or(config::API_URL_PROD);
    let url = format!("{}/generate/{}/result", base_url, job_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut token = initial_token.to_string();

    for attempt in 0..max_attempts {
        // Exponential backoff: 3s, 6s, 12s, ... capped at 30s
        let delay = std::cmp::min(3 * (1 << attempt), 30);
        tokio::time::sleep(Duration::from_secs(delay)).await;

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Poll failed: {}", e))?;

        // Token expired during polling -- refresh and retry this attempt
        if response.status().as_u16() == 401 {
            log::warn!("Poll attempt {} -- token expired, refreshing", attempt + 1);
            match auth::refresh_session(api_base).await {
                Ok(Some(session)) => {
                    token = session.access_token;
                    continue;
                }
                _ => {
                    return Err("Sitzung abgelaufen. Bitte erneut anmelden.".into());
                }
            }
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            log::warn!("Poll attempt {} -- server returned {} {}", attempt + 1, status, body);
            continue;
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Poll parse error: {}", e))?;

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
                log::info!("Poll: job {} still processing (attempt {})", job_id, attempt + 1);
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
            }
        }
    }

    Err(format!(
        "Generation job {} did not complete after {} poll attempts",
        job_id, max_attempts
    ))
}
