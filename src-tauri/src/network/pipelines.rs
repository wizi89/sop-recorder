use serde::{Deserialize, Serialize};

/// One selectable generation pipeline, as served by `GET /pipelines`.
///
/// `display_name` and `description` are user-facing copy authored in the
/// server's Langfuse, not in this build. `id` is the only thing ever sent back:
/// display names get translated and edited, and matching on one would silently
/// repoint a chain while looking like a cosmetic change.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Pipeline {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
}

/// Fetch the pipeline catalogue from the server.
///
/// The recorder never talks to Langfuse itself: it is installed on customer
/// desktops and must not hold Langfuse credentials.
///
/// An empty list is a normal, successful answer. It means the server has no
/// pipelines configured, or the feature is off, and the UI shows no selector.
pub async fn fetch_pipelines(
    access_token: &str,
    api_url: Option<&str>,
) -> Result<Vec<Pipeline>, String> {
    let base_url = api_url.unwrap_or_else(|| super::auth::api_url_for_target(None));
    let url = format!("{}/pipelines", base_url);

    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Pipeline fetch failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Pipeline fetch returned HTTP {}", response.status()));
    }

    response
        .json::<Vec<Pipeline>>()
        .await
        .map_err(|e| format!("Pipeline parse error: {}", e))
}
