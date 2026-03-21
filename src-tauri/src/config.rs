pub const API_URL_PROD: &str = "https://api.wizimate.com";
pub const API_URL_DEV: &str = "http://localhost:8000";
pub const WEBAPP_URL_PROD: &str = "https://flow.wizimate.com";
pub const WEBAPP_URL_DEV: &str = "http://localhost:3000";

pub fn webapp_url_for_target(upload_target: Option<&str>) -> &'static str {
    match upload_target {
        Some("Local") => WEBAPP_URL_DEV,
        _ => WEBAPP_URL_PROD,
    }
}
