//! Runtime configuration overrides for enterprise / self-host deployments,
//! and for pointing a development build at a backend.
//!
//! Precedence (highest first):
//!   0. Command line: `--local`, `--staging`, `--production`. A convenience
//!      spelling of the two URL variables below, for switching backend without
//!      opening Settings. It grants nothing the environment variables did not
//!      already grant, which is why it is not restricted to debug builds: a
//!      release build has to be testable against staging too.
//!   1. Environment variables: COGNICLONE_API_URL, COGNICLONE_WEBAPP_URL,
//!      COGNICLONE_UPDATER_ENABLED, COGNICLONE_ERROR_REPORTS_ENABLED
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
//!
//!      [error_reports]
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
struct FileErrorReports {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    endpoints: FileEndpoints,
    #[serde(default)]
    updater: FileUpdater,
    #[serde(default)]
    error_reports: FileErrorReports,
}

#[derive(Debug, Default)]
pub struct RuntimeOverride {
    pub api_url: Option<String>,
    pub webapp_url: Option<String>,
    pub updater_enabled: Option<bool>,
    /// `Some(false)` forces error reports off for the whole installation
    /// (design D1). Nothing else can switch them on again: the settings page
    /// shows the control disabled, and every code path treats the mode as
    /// `never`. `Some(true)` is not an override -- it restores the user's own
    /// choice, which is the default anyway.
    pub error_reports_enabled: Option<bool>,
}

impl RuntimeOverride {
    /// Whether the installation has switched error reports off.
    pub fn error_reports_forced_off(&self) -> bool {
        self.error_reports_enabled == Some(false)
    }
}

/// A backend named on the command line.
///
/// Deliberately not `upload_target` in the settings file: this leaves the
/// user's saved choice alone, so a run started with `--staging` does not change
/// what the app does the next time it is opened normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Local,
    Staging,
    Production,
}

impl Backend {
    pub fn api_url(self) -> &'static str {
        match self {
            Self::Local => crate::config::API_URL_DEV,
            Self::Staging => crate::config::API_URL_STAGING,
            Self::Production => crate::config::API_URL_PROD,
        }
    }

    pub fn webapp_url(self) -> &'static str {
        match self {
            Self::Local => crate::config::WEBAPP_URL_DEV,
            Self::Staging => crate::config::WEBAPP_URL_STAGING,
            Self::Production => crate::config::WEBAPP_URL_PROD,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

/// The backend named on the command line, if any.
///
/// Unknown arguments are ignored rather than rejected: the webview and the
/// runner both pass arguments of their own, and a launcher that refuses to
/// start over one it does not recognise would be its own bug. Two backends at
/// once *is* rejected -- picking one silently is how you end up recording
/// against production while believing you are on staging, which is exactly the
/// confusion this flag exists to remove.
pub fn parse_backend_flag(args: &[String]) -> Result<Option<Backend>, String> {
    let mut found: Option<Backend> = None;
    for arg in args {
        let backend = match arg.as_str() {
            "--local" => Backend::Local,
            "--staging" => Backend::Staging,
            "--production" | "--prod" => Backend::Production,
            _ => continue,
        };
        match found {
            Some(existing) if existing != backend => {
                return Err(format!(
                    "--{} and --{} cannot both be given; pick one backend",
                    existing.as_str(),
                    backend.as_str(),
                ));
            }
            _ => found = Some(backend),
        }
    }
    Ok(found)
}

static RUNTIME: OnceLock<RuntimeOverride> = OnceLock::new();

/// Resolve the overrides once, with `backend` taking precedence over
/// everything. Call before the first `runtime()`; later calls do nothing.
pub fn init(backend: Option<Backend>) {
    let _ = RUNTIME.set(load_from(
        default_config_path().as_deref(),
        |k| std::env::var(k).ok(),
        backend,
    ));
}

pub fn runtime() -> &'static RuntimeOverride {
    RUNTIME.get_or_init(|| {
        load_from(default_config_path().as_deref(), |k| std::env::var(k).ok(), None)
    })
}

fn default_config_path() -> Option<PathBuf> {
    dirs_next::config_dir().map(|p| p.join("CogniClone").join("config.toml"))
}

pub(crate) fn load_from<F>(
    file_path: Option<&Path>,
    env_get: F,
    backend: Option<Backend>,
) -> RuntimeOverride
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
                    out.error_reports_enabled = cfg.error_reports.enabled;
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
        out.updater_enabled = parse_bool(&v).or(out.updater_enabled);
    }
    if let Some(v) = env_get("COGNICLONE_ERROR_REPORTS_ENABLED") {
        out.error_reports_enabled = parse_bool(&v).or(out.error_reports_enabled);
    }

    // Highest precedence: an explicit flag on this run beats a file written
    // once and a variable exported in a shell profile, both of which are easy
    // to forget about.
    if let Some(backend) = backend {
        out.api_url = Some(backend.api_url().to_string());
        out.webapp_url = Some(backend.webapp_url().to_string());
    }

    out
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
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

    // --- backend flags -------------------------------------------------

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn each_flag_names_its_backend() {
        for (flag, expected) in [
            ("--local", Backend::Local),
            ("--staging", Backend::Staging),
            ("--production", Backend::Production),
            ("--prod", Backend::Production),
        ] {
            assert_eq!(
                parse_backend_flag(&args(&["cogniclone", flag])).unwrap(),
                Some(expected),
                "{}",
                flag,
            );
        }
    }

    #[test]
    fn no_flag_leaves_the_backend_alone() {
        assert_eq!(parse_backend_flag(&args(&["cogniclone"])).unwrap(), None);
    }

    /// The runner and the webview both pass arguments of their own. Refusing to
    /// start over one we do not recognise would be its own bug.
    #[test]
    fn unknown_arguments_are_ignored() {
        let parsed = parse_backend_flag(&args(&[
            "cogniclone",
            "--some-webview-flag",
            "--staging",
            "positional",
        ]))
        .unwrap();
        assert_eq!(parsed, Some(Backend::Staging));
    }

    /// Silently picking one is how you record against production while
    /// believing you are on staging -- the confusion the flag exists to remove.
    #[test]
    fn two_different_backends_are_refused() {
        let error = parse_backend_flag(&args(&["cogniclone", "--staging", "--production"]))
            .unwrap_err();
        assert!(error.contains("staging"), "{}", error);
        assert!(error.contains("production"), "{}", error);
    }

    #[test]
    fn the_same_backend_twice_is_not_a_conflict() {
        assert_eq!(
            parse_backend_flag(&args(&["cogniclone", "--staging", "--staging"])).unwrap(),
            Some(Backend::Staging),
        );
    }

    #[test]
    fn a_flag_sets_both_urls_for_that_backend() {
        let out = load_from(None, env_from(&[]), Some(Backend::Staging));
        assert_eq!(out.api_url.as_deref(), Some(crate::config::API_URL_STAGING));
        assert_eq!(
            out.webapp_url.as_deref(),
            Some(crate::config::WEBAPP_URL_STAGING),
        );
    }

    /// An exported variable in a shell profile, or a config file written once,
    /// is easy to forget. A flag typed on this run is not.
    #[test]
    fn a_flag_beats_the_environment_and_the_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[endpoints]\napi_url = \"https://file.example\"\n",
        )
        .unwrap();

        let out = load_from(
            Some(&path),
            env_from(&[("COGNICLONE_API_URL", "https://env.example")]),
            Some(Backend::Local),
        );
        assert_eq!(out.api_url.as_deref(), Some(crate::config::API_URL_DEV));
    }

    /// The flag only moves the endpoints. Anything else the installation set --
    /// notably error reports being switched off -- still stands.
    #[test]
    fn a_flag_does_not_disturb_the_other_overrides() {
        let out = load_from(
            None,
            env_from(&[("COGNICLONE_ERROR_REPORTS_ENABLED", "off")]),
            Some(Backend::Staging),
        );
        assert!(out.error_reports_forced_off());
    }

    #[test]
    fn defaults_when_nothing_set() {
        let out = load_from(None, env_from(&[]), None);
        assert!(out.api_url.is_none());
        assert!(out.webapp_url.is_none());
        assert!(out.updater_enabled.is_none());
        assert!(out.error_reports_enabled.is_none());
        assert!(!out.error_reports_forced_off());
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

        let out = load_from(Some(&path), env_from(&[]), None);
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
            None,
        );
        assert_eq!(out.api_url.as_deref(), Some("https://env.example"));
    }

    #[test]
    fn malformed_toml_ignored() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not valid toml ===").unwrap();
        let out = load_from(Some(&path), env_from(&[]), None);
        assert!(out.api_url.is_none());
        assert!(out.webapp_url.is_none());
        assert!(out.updater_enabled.is_none());
    }

    #[test]
    fn updater_env_parses_truthy_values() {
        let out = load_from(None, env_from(&[("COGNICLONE_UPDATER_ENABLED", "false")]), None);
        assert_eq!(out.updater_enabled, Some(false));

        let out = load_from(None, env_from(&[("COGNICLONE_UPDATER_ENABLED", "1")]), None);
        assert_eq!(out.updater_enabled, Some(true));

        let out = load_from(
            None,
            env_from(&[("COGNICLONE_UPDATER_ENABLED", "garbage")]),
            None,
        );
        assert_eq!(out.updater_enabled, None);
    }

    #[test]
    fn missing_file_is_ok() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        let out = load_from(Some(&path), env_from(&[]), None);
        assert!(out.api_url.is_none());
    }

    #[test]
    fn error_reports_switched_off_by_the_config_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[error_reports]
enabled = false
"#,
        )
        .unwrap();

        let out = load_from(Some(&path), env_from(&[]), None);
        assert_eq!(out.error_reports_enabled, Some(false));
        assert!(out.error_reports_forced_off());
    }

    #[test]
    fn error_reports_switched_off_by_the_environment() {
        let out = load_from(
            None,
            env_from(&[("COGNICLONE_ERROR_REPORTS_ENABLED", "off")]),
            None,
        );
        assert!(out.error_reports_forced_off());
    }

    #[test]
    fn error_reports_env_overrides_the_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[error_reports]
enabled = false
"#,
        )
        .unwrap();

        let out = load_from(
            Some(&path),
            env_from(&[("COGNICLONE_ERROR_REPORTS_ENABLED", "true")]),
            None,
        );
        assert_eq!(out.error_reports_enabled, Some(true));
        assert!(!out.error_reports_forced_off());
    }

    #[test]
    fn a_malformed_error_reports_value_leaves_the_file_value_standing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[error_reports]
enabled = false
"#,
        )
        .unwrap();

        let out = load_from(
            Some(&path),
            env_from(&[("COGNICLONE_ERROR_REPORTS_ENABLED", "vielleicht")]),
            None,
        );
        assert_eq!(out.error_reports_enabled, Some(false));

        // And with nothing in the file, a malformed value is simply ignored.
        let out = load_from(
            None,
            env_from(&[("COGNICLONE_ERROR_REPORTS_ENABLED", "vielleicht")]),
            None,
        );
        assert!(out.error_reports_enabled.is_none());
        assert!(!out.error_reports_forced_off());
    }
}
