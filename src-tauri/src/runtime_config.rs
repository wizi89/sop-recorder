//! Runtime configuration overrides for enterprise / self-host deployments.
//!
//! Precedence (highest first):
//!   1. Environment variables: COGNICLONE_API_URL, COGNICLONE_WEBAPP_URL,
//!      COGNICLONE_UPDATER_ENABLED
//!   2. Config file at the OS user config dir. On Windows that resolves to
//!      `%APPDATA%\CogniClone\config.toml`. Example contents:
//!
//!      ```toml
//!      [endpoints]
//!      api_url = "https://api.gebit.local"
//!      webapp_url = "https://sop.gebit.local"
//!
//!      [updater]
//!      enabled = false
//!      ```
//!
//!   3. Compile-time defaults in `crate::config`.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Default, Deserialize)]
struct FileEndpoints {
    api_url: Option<String>,
    webapp_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FileUpdater {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    endpoints: FileEndpoints,
    #[serde(default)]
    updater: FileUpdater,
}

#[derive(Debug, Default)]
pub struct RuntimeOverride {
    pub api_url: Option<String>,
    pub webapp_url: Option<String>,
    pub updater_enabled: Option<bool>,
}

static RUNTIME: OnceLock<RuntimeOverride> = OnceLock::new();

pub fn runtime() -> &'static RuntimeOverride {
    RUNTIME.get_or_init(|| {
        load_from(default_config_path().as_deref(), |k| std::env::var(k).ok())
    })
}

fn default_config_path() -> Option<PathBuf> {
    dirs_next::config_dir().map(|p| p.join("CogniClone").join("config.toml"))
}

pub(crate) fn load_from<F>(file_path: Option<&Path>, env_get: F) -> RuntimeOverride
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = RuntimeOverride::default();

    if let Some(path) = file_path {
        if let Ok(text) = std::fs::read_to_string(path) {
            match toml::from_str::<FileConfig>(&text) {
                Ok(cfg) => {
                    out.api_url = cfg.endpoints.api_url;
                    out.webapp_url = cfg.endpoints.webapp_url;
                    out.updater_enabled = cfg.updater.enabled;
                }
                Err(err) => {
                    log::warn!(
                        "ignoring malformed CogniClone config.toml at {}: {}",
                        path.display(),
                        err
                    );
                }
            }
        }
    }

    if let Some(v) = env_get("COGNICLONE_API_URL") {
        out.api_url = Some(v);
    }
    if let Some(v) = env_get("COGNICLONE_WEBAPP_URL") {
        out.webapp_url = Some(v);
    }
    if let Some(v) = env_get("COGNICLONE_UPDATER_ENABLED") {
        out.updater_enabled = match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => out.updater_enabled,
        };
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn defaults_when_nothing_set() {
        let out = load_from(None, env_from(&[]));
        assert!(out.api_url.is_none());
        assert!(out.webapp_url.is_none());
        assert!(out.updater_enabled.is_none());
    }

    #[test]
    fn file_values_picked_up() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[endpoints]
api_url = "https://api.gebit.local"
webapp_url = "https://sop.gebit.local"

[updater]
enabled = false
"#,
        )
        .unwrap();

        let out = load_from(Some(&path), env_from(&[]));
        assert_eq!(out.api_url.as_deref(), Some("https://api.gebit.local"));
        assert_eq!(out.webapp_url.as_deref(), Some("https://sop.gebit.local"));
        assert_eq!(out.updater_enabled, Some(false));
    }

    #[test]
    fn env_overrides_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[endpoints]
api_url = "https://file.example"
"#,
        )
        .unwrap();

        let out = load_from(
            Some(&path),
            env_from(&[("COGNICLONE_API_URL", "https://env.example")]),
        );
        assert_eq!(out.api_url.as_deref(), Some("https://env.example"));
    }

    #[test]
    fn malformed_toml_ignored() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not valid toml ===").unwrap();
        let out = load_from(Some(&path), env_from(&[]));
        assert!(out.api_url.is_none());
        assert!(out.webapp_url.is_none());
        assert!(out.updater_enabled.is_none());
    }

    #[test]
    fn updater_env_parses_truthy_values() {
        let out = load_from(None, env_from(&[("COGNICLONE_UPDATER_ENABLED", "false")]));
        assert_eq!(out.updater_enabled, Some(false));

        let out = load_from(None, env_from(&[("COGNICLONE_UPDATER_ENABLED", "1")]));
        assert_eq!(out.updater_enabled, Some(true));

        let out = load_from(
            None,
            env_from(&[("COGNICLONE_UPDATER_ENABLED", "garbage")]),
        );
        assert_eq!(out.updater_enabled, None);
    }

    #[test]
    fn missing_file_is_ok() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        let out = load_from(Some(&path), env_from(&[]));
        assert!(out.api_url.is_none());
    }
}
