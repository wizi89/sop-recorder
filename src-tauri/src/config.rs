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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webapp_url_defaults_to_prod() {
        assert_eq!(webapp_url_for_target(None), WEBAPP_URL_PROD);
    }

    #[test]
    fn webapp_url_local_target_returns_dev() {
        assert_eq!(webapp_url_for_target(Some("Local")), WEBAPP_URL_DEV);
    }

    #[test]
    fn webapp_url_production_target_returns_prod() {
        assert_eq!(webapp_url_for_target(Some("Production")), WEBAPP_URL_PROD);
    }

    #[test]
    fn webapp_url_unknown_target_returns_prod() {
        assert_eq!(webapp_url_for_target(Some("unknown")), WEBAPP_URL_PROD);
    }
}
