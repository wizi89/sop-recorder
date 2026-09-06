use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_store::StoreExt;

const STORE_FILENAME: &str = "settings.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub output_dir: String,
    pub logs_dir: String,
    pub hide_from_screenshots: bool,
    #[serde(default)]
    pub upload_target: Option<String>,
    #[serde(default)]
    pub skip_pii_check: bool,
    #[serde(default = "default_pipeline_version")]
    pub pipeline_version: u8,
    #[serde(default = "default_generation_model")]
    pub generation_model: String,
    /// `ask`, `always` or `never` (design D1). Defaults to `ask`, so a
    /// settings file written by an older build loads as "ask before sending"
    /// rather than as a mode the user never chose.
    #[serde(default = "default_error_reports")]
    pub error_reports: String,
}

/// The value `logs_dir` should be corrected to, or `None` when the stored one
/// is already right.
///
/// The log directory is not a user choice -- nothing reads the stored value,
/// and the log plugin's target is fixed before an `AppHandle` exists -- so a
/// settings file that disagrees with reality is simply wrong, and is corrected
/// at startup rather than left until the next save.
fn logs_dir_correction(stored: Option<&str>, actual: &str) -> Option<String> {
    (stored != Some(actual)).then(|| actual.to_string())
}

fn default_pipeline_version() -> u8 {
    1
}

fn default_generation_model() -> String {
    "azure/gpt-4.1".to_string()
}

fn default_error_reports() -> String {
    crate::error_reports::ReportMode::Ask.as_str().to_string()
}

impl AppSettings {
    /// Write defaults to the store if no settings have been saved yet.
    /// For upgrades from older versions, preserves the legacy workflows folder
    /// if it already contains recordings.
    pub fn initialize(app: &tauri::AppHandle) {
        let Ok(store) = tauri_plugin_store::StoreExt::store(app, STORE_FILENAME) else {
            return;
        };
        let mut defaults = Self::defaults(app);

        // If output_dir already exists in the store, settings were previously
        // saved -- but the log directory is not the user's choice and a file
        // written by an earlier build may name a directory nothing writes to.
        // Corrected here rather than only on the next save, so the file and the
        // application never disagree about where the logs are.
        if store.get("output_dir").is_some() {
            let stored = store.get("logs_dir").and_then(|v| v.as_str().map(String::from));
            if let Some(correction) = logs_dir_correction(stored.as_deref(), &defaults.logs_dir) {
                log::info!(
                    "Correcting the stored log directory from {:?} to {}",
                    stored, correction,
                );
                store.set("logs_dir", serde_json::json!(correction));
            }
            return;
        }

        // TODO(cleanup): Remove this legacy migration once all users have updated
        // past v0.8.x. Added 2026-03-31.
        let legacy_dir = dirs_next::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("CogniClone Workflows");
        if legacy_dir.is_dir() {
            defaults.output_dir = legacy_dir.to_string_lossy().to_string();
        }

        store.set("output_dir", serde_json::json!(defaults.output_dir));
        store.set("logs_dir", serde_json::json!(defaults.logs_dir));
        store.set("hide_from_screenshots", serde_json::json!(defaults.hide_from_screenshots));
        store.set("skip_pii_check", serde_json::json!(defaults.skip_pii_check));
        store.set("pipeline_version", serde_json::json!(defaults.pipeline_version));
        store.set("generation_model", serde_json::json!(defaults.generation_model));
        store.set("error_reports", serde_json::json!(defaults.error_reports));
    }

    pub fn defaults(app: &tauri::AppHandle) -> Self {
        let product_name = &app.config().product_name;
        let name = product_name.as_deref().unwrap_or("cogniclone");

        let docs = dirs_next::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(format!("{} Workflows", name));
        // The directory the log plugin actually writes to. `tauri-plugin-log`
        // defaults to `TargetKind::LogDir`, which resolves to `app_log_dir()`;
        // this used to derive its own path from `app_local_data_dir()/logs`
        // instead. The two coincide on Windows and differ on macOS
        // (`~/Library/Logs/<id>` against `~/Library/Application Support/<id>/logs`),
        // which is why settings.json named a directory that did not exist and
        // only the macOS test caught it.
        let app_data = app.path().app_log_dir()
            .unwrap_or_else(|_| dirs_next::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(name)
                .join("logs"));

        Self {
            output_dir: docs.to_string_lossy().to_string(),
            logs_dir: app_data.to_string_lossy().to_string(),
            hide_from_screenshots: true,
            upload_target: None,
            skip_pii_check: false,
            pipeline_version: 1,
            generation_model: default_generation_model(),
            error_reports: default_error_reports(),
        }
    }
}

/// Push the current settings into `crate::error_reports`, which holds them in
/// lock-free globals so the panic hook can read them without touching the
/// store (design D5).
pub fn publish_error_report_context(settings: &AppSettings) {
    crate::error_reports::set_mode(crate::error_reports::resolve_mode(
        Some(settings.error_reports.as_str()),
        crate::runtime_config::runtime().error_reports_forced_off(),
    ));
    crate::error_reports::set_context(crate::error_reports::ReportContext {
        settings: crate::error_reports::ReportSettings {
            upload_target: settings.upload_target.clone(),
            pipeline_version: settings.pipeline_version,
            generation_model: settings.generation_model.clone(),
            hide_from_screenshots: settings.hide_from_screenshots,
            skip_pii_check: settings.skip_pii_check,
        },
        output_dir: Some(settings.output_dir.clone()),
    });
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let store = app.store(STORE_FILENAME).map_err(|e| e.to_string())?;
    let defaults = AppSettings::defaults(&app);

    let output_dir = store
        .get("output_dir")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or(defaults.output_dir);
    let logs_dir = store
        .get("logs_dir")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or(defaults.logs_dir);
    let hide_from_screenshots = store
        .get("hide_from_screenshots")
        .and_then(|v| v.as_bool())
        .unwrap_or(defaults.hide_from_screenshots);

    let upload_target = store
        .get("upload_target")
        .and_then(|v| v.as_str().map(String::from))
        .filter(|target| matches!(target.as_str(), "Local" | "Staging"));

    let skip_pii_check = store
        .get("skip_pii_check")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let pipeline_version = store
        .get("pipeline_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u8)
        .unwrap_or(1);

    let generation_model = store
        .get("generation_model")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(default_generation_model);

    let error_reports = store
        .get("error_reports")
        .and_then(|v| v.as_str().map(String::from))
        .filter(|mode| matches!(mode.as_str(), "ask" | "always" | "never"))
        .unwrap_or_else(default_error_reports);

    Ok(AppSettings {
        output_dir,
        logs_dir,
        hide_from_screenshots,
        upload_target,
        skip_pii_check,
        pipeline_version,
        generation_model,
        error_reports,
    })
}

#[tauri::command]
pub async fn save_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let store = app.store(STORE_FILENAME).map_err(|e| e.to_string())?;

    store.set("output_dir", serde_json::json!(settings.output_dir));
    store.set("logs_dir", serde_json::json!(settings.logs_dir));
    store.set(
        "hide_from_screenshots",
        serde_json::json!(settings.hide_from_screenshots),
    );

    if let Some(target) = &settings.upload_target {
        if matches!(target.as_str(), "Local" | "Staging") {
            store.set("upload_target", serde_json::json!(target));
        } else {
            store.delete("upload_target");
        }
    } else {
        store.delete("upload_target");
    }

    store.set("skip_pii_check", serde_json::json!(settings.skip_pii_check));
    store.set("pipeline_version", serde_json::json!(settings.pipeline_version));
    store.set("generation_model", serde_json::json!(settings.generation_model));
    if matches!(settings.error_reports.as_str(), "ask" | "always" | "never") {
        store.set("error_reports", serde_json::json!(settings.error_reports));
    }

    // Written through before this returns, rather than left to the store
    // plugin's 100 ms debounce. The settings window closes the moment this
    // resolves, and a save the user watched complete has to be on disk: an
    // unflushed write is indistinguishable from a successful one, which is
    // half of why the 2026-09-03 test spent an hour on a wrong assumption
    // about the app's state.
    store.save().map_err(|e| {
        log::error!("Failed to write settings to disk: {}", e);
        format!("Einstellungen konnten nicht gespeichert werden: {}", e)
    })?;

    // The mode and the settings subset a report carries have both just
    // changed, and the panic hook reads them from memory rather than from the
    // store, so the copy it reads has to be refreshed here.
    publish_error_report_context(&settings);

    Ok(())
}

/// Whether a BYOK key is stored, without retrieving it.
///
/// The settings window only ever needs the answer to this question, and asking
/// it this way keeps the credential store off the window's load path. Reading
/// the key there made opening settings wait on the OS credential store, which
/// after a reinstall or a re-signing can prompt or block for seconds -- long
/// enough for the user to change a setting into a form that was about to be
/// overwritten by the load.
#[tauri::command]
pub async fn has_api_key() -> bool {
    crate::network::auth::keyring_load("openai-key")
        .ok()
        .flatten()
        .is_some_and(|key| !key.is_empty())
}

#[tauri::command]
pub async fn get_webapp_url(app: tauri::AppHandle) -> Result<String, String> {
    let target = app
        .store(STORE_FILENAME)
        .ok()
        .and_then(|store| {
            store
                .get("upload_target")
                .and_then(|v| v.as_str().map(String::from))
        });
    Ok(crate::config::webapp_url_for_target(target.as_deref()).to_string())
}

/// Whether the installation has switched error reports off (design D1). The
/// settings page disables its control and shows a note when this is true.
#[tauri::command]
pub async fn are_error_reports_forced_off() -> bool {
    crate::runtime_config::runtime().error_reports_forced_off()
}

#[tauri::command]
pub async fn is_updater_enabled() -> bool {
    crate::runtime_config::runtime()
        .updater_enabled
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This module's executable source: no tests, no comments.
    ///
    /// The guards below assert that certain calls are *absent*, and this file
    /// is read whole by `include_str!`. Both exclusions are load-bearing. The
    /// tests would otherwise match the very string they assert against, in
    /// their own failure messages; and a comment naming the call that was
    /// removed -- which is exactly what the code above the change carries, to
    /// explain why -- would read as the call still being there.
    ///
    /// Line endings are normalised first: git hands Windows checkouts CRLF.
    fn production_source() -> String {
        let source = include_str!("settings.rs").replace("\r\n", "\n");
        let tests_begin = source.find("\n#[cfg(test)]").expect("no test module");
        source[..tests_begin]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Opening the settings window must not wait on the OS credential store.
    ///
    /// A source guard rather than a behavioural test: the property is the
    /// *absence* of a call, and the keyring has no seam to stub here. Reading
    /// the key on this path is what made the window load slowly enough after a
    /// reinstall for the pending load to overwrite an edit the user had already
    /// made (2026-09-03). `has_api_key` answers the only question the window
    /// asks, and is called on its own.
    #[test]
    fn the_settings_load_path_never_touches_the_credential_store() {
        let source = production_source();
        let start = source
            .find("pub async fn get_settings")
            .expect("get_settings not found");
        let end = source[start..]
            .find("\n#[tauri::command]")
            .map(|offset| start + offset)
            .unwrap_or(source.len());
        let body = &source[start..end];

        assert!(
            !body.contains("keyring"),
            "get_settings must not read the credential store:\n{}",
            body,
        );
    }

    /// A save has to reach the disk before it reports success, because the
    /// settings window closes on that success and the user reads the file as
    /// the record of what they chose.
    #[test]
    fn saving_settings_flushes_the_store_to_disk() {
        let source = production_source();
        let start = source
            .find("pub async fn save_settings")
            .expect("save_settings not found");
        let end = source[start..]
            .find("\n#[tauri::command]")
            .map(|offset| start + offset)
            .unwrap_or(source.len());
        let body = &source[start..end];

        assert!(
            body.contains("store.save()"),
            "save_settings must flush rather than rely on the autosave debounce",
        );
    }

    /// `logs_dir` has to name the directory the log plugin writes to, on every
    /// platform. It used to be derived from `app_local_data_dir()`, which
    /// coincides with the log directory on Windows and does not on macOS -- so
    /// settings.json named a directory that did not exist, and only the macOS
    /// test caught it.
    ///
    /// A source guard: resolving either path for real needs a running Tauri
    /// app. What is asserted is that the derivation is the log plugin's own.
    #[test]
    fn the_reported_log_directory_is_the_one_the_log_plugin_uses() {
        let source = production_source();

        assert!(
            source.contains("app.path().app_log_dir()"),
            "logs_dir must come from app_log_dir(), the log plugin's own target",
        );
        assert!(
            !source.contains("app_local_data_dir()"),
            "app_local_data_dir() is not the log directory on macOS",
        );
    }

    #[test]
    fn a_stale_log_directory_is_corrected() {
        // What the 2026-09-03 test found in settings.json on macOS.
        assert_eq!(
            logs_dir_correction(
                Some("/Users/m/Library/Application Support/com.cogniclone.recorder/logs"),
                "/Users/m/Library/Logs/com.cogniclone.recorder",
            ),
            Some("/Users/m/Library/Logs/com.cogniclone.recorder".to_string()),
        );
    }

    #[test]
    fn a_correct_log_directory_is_left_alone() {
        let actual = "/Users/m/Library/Logs/com.cogniclone.recorder";
        assert_eq!(logs_dir_correction(Some(actual), actual), None);
    }

    #[test]
    fn a_settings_file_with_no_log_directory_gets_one() {
        let actual = "/Users/m/Library/Logs/com.cogniclone.recorder";
        assert_eq!(logs_dir_correction(None, actual), Some(actual.to_string()));
    }

    /// The BYOK key is no longer round-tripped through `AppSettings`. It used
    /// to be loaded on `get_settings` and written back on `save_settings`,
    /// which meant a save made while the load had failed -- the window's own
    /// `.catch(() => {})` -- cleared the user's key. Nothing writes or clears
    /// it now, so it simply survives.
    #[test]
    fn settings_carry_no_credential() {
        let stored = serde_json::json!({
            "output_dir": "/tmp/out",
            "logs_dir": "/tmp/logs",
            "hide_from_screenshots": true,
            "api_key": "sk-should-be-ignored"
        });
        let settings: AppSettings = serde_json::from_value(stored).unwrap();
        let round_tripped = serde_json::to_value(&settings).unwrap();

        assert!(
            round_tripped.get("api_key").is_none(),
            "a credential must not travel in the settings payload: {}",
            round_tripped,
        );
    }

    /// A settings file written before this change has no `error_reports` key.
    /// It has to load as "ask" -- the default the user would have been given
    /// -- rather than failing to parse or silently arriving at `never`.
    #[test]
    fn settings_from_an_older_build_default_to_ask() {
        let stored = serde_json::json!({
            "output_dir": "/Users/anna/Documents/cogniclone Workflows",
            "logs_dir": "/Users/anna/Library/Logs/com.cogniclone.recorder",
            "hide_from_screenshots": true,
            "api_key": null
        });  // an older build's file; the field is ignored now

        let settings: AppSettings = serde_json::from_value(stored).unwrap();
        assert_eq!(settings.error_reports, "ask");
        assert_eq!(
            crate::error_reports::resolve_mode(Some(&settings.error_reports), false),
            crate::error_reports::ReportMode::Ask
        );
    }

    #[test]
    fn a_saved_mode_survives_the_round_trip() {
        for mode in ["ask", "always", "never"] {
            let stored = serde_json::json!({
                "output_dir": "",
                "logs_dir": "",
                "hide_from_screenshots": true,
                "api_key": null,
                "error_reports": mode
            });
            let settings: AppSettings = serde_json::from_value(stored).unwrap();
            assert_eq!(settings.error_reports, mode);
        }
    }
}
