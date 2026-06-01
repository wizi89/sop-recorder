pub const API_URL_PROD: &str = "https://api.wizimate.com";
pub const API_URL_DEV: &str = "http://localhost:8000";
pub const API_URL_STAGING: &str = "https://api.staging.cogniclone.ai";
pub const WEBAPP_URL_PROD: &str = "https://app.cogniclone.ai";
pub const WEBAPP_URL_DEV: &str = "http://localhost:3000";
pub const WEBAPP_URL_STAGING: &str = "https://staging.cogniclone.ai";

/// Pure mapping from upload target to compile-time default URL. Hermetic:
/// does not consult environment or config file. Used by tests; callers should
/// prefer `webapp_url_for_target`, which also honors runtime overrides.
fn webapp_url_default(upload_target: Option<&str>) -> &'static str {
    match upload_target {
        Some("Local") => WEBAPP_URL_DEV,
        Some("Staging") => WEBAPP_URL_STAGING,
        _ => WEBAPP_URL_PROD,
    }
}

pub fn webapp_url_for_target(upload_target: Option<&str>) -> &'static str {
    if let Some(url) = crate::runtime_config::runtime().webapp_url.as_deref() {
        return url;
    }
    webapp_url_default(upload_target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webapp_url_defaults_to_prod() {
        assert_eq!(webapp_url_default(None), WEBAPP_URL_PROD);
    }

    #[test]
    fn webapp_url_local_target_returns_dev() {
        assert_eq!(webapp_url_default(Some("Local")), WEBAPP_URL_DEV);
    }

    #[test]
    fn webapp_url_staging_target_returns_staging() {
        assert_eq!(webapp_url_default(Some("Staging")), WEBAPP_URL_STAGING);
    }

    #[test]
    fn webapp_url_production_target_returns_prod() {
        assert_eq!(webapp_url_default(Some("Production")), WEBAPP_URL_PROD);
    }

    #[test]
    fn webapp_url_unknown_target_returns_prod() {
        assert_eq!(webapp_url_default(Some("unknown")), WEBAPP_URL_PROD);
    }
}
