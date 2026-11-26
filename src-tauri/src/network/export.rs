use std::future::Future;
use std::path::Path;
use std::time::Duration;

use super::auth;

/// What a PDF download ended in, when it reached the server.
#[derive(Debug, PartialEq, Eq)]
pub enum PdfOutcome {
    /// The PDF is written to `<dest>/guide.pdf`.
    Saved,
    /// The server has no PDF for this guide (404): nothing is written.
    NotFound,
}

/// Fetch the guide's PDF from the server and save it as `<dest>/guide.pdf`.
pub async fn download_pdf(
    token: &str,
    pdf_url: &str,
    api_url: Option<&str>,
    api_base: &str,
    dest: &Path,
) -> Result<PdfOutcome, String> {
    download_pdf_with(token, pdf_url, api_url, dest, || async {
        auth::refresh_session(api_base)
            .await
            .map(|s| s.map(|s| s.access_token))
    })
    .await
}

/// `refresh` is called at most once, after a 401, and yields the new access
/// token (`None`: no session to refresh).
async fn download_pdf_with<F, Fut>(
    token: &str,
    pdf_url: &str,
    api_url: Option<&str>,
    dest: &Path,
    refresh: F,
) -> Result<PdfOutcome, String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<String>, String>>,
{
    // The bearer goes only to the configured API base, never to a host taken
    // from a server response.
    if !pdf_url.starts_with('/') || pdf_url.starts_with("//") {
        return Err(format!("pdf_url is not an API path: {}", pdf_url));
    }
    let base_url = api_url.unwrap_or_else(|| auth::api_url_for_target(None));
    let url = format!("{}{}", base_url.trim_end_matches('/'), pdf_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;
    let get = |token: &str| {
        client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
    };

    let mut response = get(token)
        .await
        .map_err(|e| format!("PDF download request failed: {}", e))?;
    if response.status().as_u16() == 401 {
        let fresh = refresh()
            .await?
            .ok_or_else(|| "PDF download returned 401 and no session to refresh".to_string())?;
        response = get(&fresh)
            .await
            .map_err(|e| format!("PDF download request failed: {}", e))?;
    }

    let status = response.status();
    if status.as_u16() == 404 {
        return Ok(PdfOutcome::NotFound);
    }
    if !status.is_success() {
        return Err(format!("PDF download returned status {}", status));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read PDF response body: {}", e))?;
    std::fs::write(dest.join("guide.pdf"), &bytes)
        .map_err(|e| format!("Failed to write PDF file: {}", e))?;
    Ok(PdfOutcome::Saved)
}

#[cfg(test)]
mod tests;
