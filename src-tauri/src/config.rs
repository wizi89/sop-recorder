pub const API_URL_PROD: &str = "https://api.wizimate.com";
pub const API_URL_DEV: &str = "http://localhost:8000";
pub const API_URL_STAGING: &str = "https://api.staging.cogniclone.ai";
pub const WEBAPP_URL_PROD: &str = "https://app.cogniclone.ai";
pub const WEBAPP_URL_DEV: &str = "http://localhost:3000";
pub const WEBAPP_URL_STAGING: &str = "https://staging.cogniclone.ai";

/// Stable channel. GitHub's `releases/latest` deliberately skips prereleases,
/// so a beta tag can never reach a user on this endpoint.
pub const UPDATE_ENDPOINT_STABLE: &str =
    "https://github.com/wizi89/sop-recorder/releases/latest/download/latest.json";
/// Beta channel. A rolling `beta` release that CI clobbers on every tag,
/// stable ones included -- otherwise a tester who took 0.19.0-rc.1 would be
/// stranded below 0.19.0, which sorts higher under semver and would never be
/// offered on a channel that only carried prereleases.
pub const UPDATE_ENDPOINT_BETA: &str =
    "https://github.com/wizi89/sop-recorder/releases/download/beta/latest.json";

pub fn update_endpoint(beta: bool) -> &'static str {
    if beta {
        UPDATE_ENDPOINT_BETA
    } else {
        UPDATE_ENDPOINT_STABLE
    }
}

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
    fn update_endpoint_defaults_to_stable() {
        assert_eq!(update_endpoint(false), UPDATE_ENDPOINT_STABLE);
        assert_eq!(update_endpoint(true), UPDATE_ENDPOINT_BETA);
    }

    /// The stable endpoint has to stay on `releases/latest`: that is the only
    /// thing keeping prereleases away from users who did not opt in.
    #[test]
    fn stable_endpoint_uses_releases_latest() {
        assert!(UPDATE_ENDPOINT_STABLE.contains("/releases/latest/"));
        assert!(!UPDATE_ENDPOINT_BETA.contains("/releases/latest/"));
    }

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
