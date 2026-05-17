use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, serde::Serialize, Default)]
pub struct OrgFeatures {
    #[serde(default)]
    pub advanced_settings: bool,
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct Quota {
    pub count: i64,
    pub limit: i64,
    pub remaining: i64,
    #[serde(default)]
    pub features: OrgFeatures,
    #[serde(default)]
    pub generation_settings: GenerationSettings,
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct GenerationSettings {
    #[serde(default)]
    pub pipeline_versions: Vec<u8>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub default_model: String,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            pipeline_versions: vec![1, 2],
            models: vec!["azure/gpt-4.1".to_string()],
            default_model: "azure/gpt-4.1".to_string(),
        }
    }
}

/// Fetch the current user's generation quota from the server.
///
/// Uses `GET /quota` with a Bearer token. Caller is responsible for
/// providing a non-expired access token (refresh before calling if in doubt).
pub async fn fetch_quota(access_token: &str, api_url: Option<&str>) -> Result<Quota, String> {
    let base_url = api_url.unwrap_or_else(|| super::auth::api_url_for_target(None));
    let url = format!("{}/quota", base_url);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Quota fetch failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Quota fetch returned HTTP {}",
            response.status()
        ));
    }

    response
        .json::<Quota>()
        .await
        .map_err(|e| format!("Quota parse error: {}", e))
}
