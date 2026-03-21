use reqwest::multipart;
use std::path::Path;
use std::time::Duration;

const UPLOAD_TIMEOUT_SECS: u64 = 300;
const API_URL_PROD: &str = "https://api.wizimate.com";

/// Upload audio + screenshots to the generation endpoint.
/// Returns the SSE stream URL or response.
pub async fn upload_multipart(
    access_token: &str,
    openai_key: Option<&str>,
    audio_path: &Path,
    screenshot_paths: &[(u32, &Path)],
    guide_title: &str,
    api_url: Option<&str>,
) -> Result<reqwest::Response, String> {
    let base_url = api_url.unwrap_or(API_URL_PROD);
    let url = format!("{}/generate", base_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(UPLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut form = multipart::Form::new();

    // Audio file
    let audio_bytes = std::fs::read(audio_path)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;
    let audio_part = multipart::Part::bytes(audio_bytes)
        .file_name("recording.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    form = form.part("audio", audio_part);

    // Screenshots
    for (order, path) in screenshot_paths {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read screenshot {}: {}", order, e))?;
        let field_name = format!("step_{:02}", order);
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.png", field_name));
        let part = multipart::Part::bytes(bytes)
            .file_name(file_name)
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;
        form = form.part(field_name, part);
    }

    // Metadata
    let metadata = serde_json::json!({
        "guide_title": guide_title,
        "step_count": screenshot_paths.len(),
    });
    form = form.text("metadata", metadata.to_string());

    let mut req = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .multipart(form);

    if let Some(key) = openai_key {
        req = req.header("X-OpenAI-Key", key);
    }

    let response = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "Request timed out (300s)".to_string()
        } else if e.is_connect() {
            "Server not reachable. Check your connection.".to_string()
        } else {
            format!("Upload failed: {}", e)
        }
    })?;

    let status = response.status();
    log::info!("Upload response: {} {}", status.as_u16(), status.canonical_reason().unwrap_or(""));

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::error!("Upload failed: {} - {}", status, body);
        return Err(format!("Server error ({}): {}", status, body));
    }

    // For SSE, we need to return the response to stream events
    // Re-send as SSE request... actually the /generate endpoint returns SSE directly
    // So we return the response for the SSE consumer
    Ok(response)
}

/// Retry logic with exponential backoff for transient errors.
pub async fn upload_with_retry(
    access_token: &str,
    openai_key: Option<&str>,
    audio_path: &Path,
    screenshot_paths: &[(u32, &Path)],
    guide_title: &str,
    api_url: Option<&str>,
    max_retries: u32,
) -> Result<reqwest::Response, String> {
    let mut last_err = String::new();
    let delays = [1, 2, 4]; // seconds

    for attempt in 0..=max_retries {
        match upload_multipart(
            access_token,
            openai_key,
            audio_path,
            screenshot_paths,
            guide_title,
            api_url,
        )
        .await
        {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                last_err = e.clone();

                // Don't retry 4xx errors (except if it looks like a transient issue)
                if e.contains("(4") && !e.contains("(429)") {
                    return Err(e);
                }

                if attempt < max_retries {
                    let delay = delays.get(attempt as usize).copied().unwrap_or(4);
                    log::warn!(
                        "Upload attempt {} failed: {}. Retrying in {}s...",
                        attempt + 1,
                        e,
                        delay
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }
    }

    Err(last_err)
}
