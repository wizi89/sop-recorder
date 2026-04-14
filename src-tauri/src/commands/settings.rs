use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_store::StoreExt;

const STORE_FILENAME: &str = "settings.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub output_dir: String,
    pub logs_dir: String,
    pub hide_from_screenshots: bool,
    pub api_key: Option<String>,
    #[serde(default)]
    pub upload_target: Option<String>,
    #[serde(default)]
    pub skip_pii_check: bool,
    #[serde(default = "default_pipeline_version")]
    pub pipeline_version: u8,
}

fn default_pipeline_version() -> u8 {
    1
}

impl AppSettings {
    /// Write defaults to the store if no settings have been saved yet.
    /// For upgrades from older versions, preserves the legacy workflows folder
    /// if it already contains recordings.
    pub fn initialize(app: &tauri::AppHandle) {
        let Ok(store) = tauri_plugin_store::StoreExt::store(app, STORE_FILENAME) else {
            return;
        };
        // If output_dir already exists in the store, settings were previously saved
        if store.get("output_dir").is_some() {
            return;
        }
        let mut defaults = Self::defaults(app);

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
    }

    pub fn defaults(app: &tauri::AppHandle) -> Self {
        let product_name = &app.config().product_name;
        let name = product_name.as_deref().unwrap_or("cogniclone");

        let docs = dirs_next::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(format!("{} Workflows", name));
        let app_data = app.path().app_local_data_dir()
            .unwrap_or_else(|_| dirs_next::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(name))
            .join("logs");

        Self {
            output_dir: docs.to_string_lossy().to_string(),
            logs_dir: app_data.to_string_lossy().to_string(),
            hide_from_screenshots: true,
            api_key: None,
            upload_target: None,
            skip_pii_check: false,
            pipeline_version: 1,
        }
    }
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
        .and_then(|v| v.as_str().map(String::from));

    let skip_pii_check = store
        .get("skip_pii_check")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let pipeline_version = store
        .get("pipeline_version")
        .and_then(|v| v.as_u64())
        .map(|v| v as u8)
        .unwrap_or(1);

    // API key is stored in keyring, not in the store
    let api_key = crate::network::auth::keyring_load("openai-key").ok().flatten();

    Ok(AppSettings {
        output_dir,
        logs_dir,
        hide_from_screenshots,
        api_key,
        upload_target,
        skip_pii_check,
        pipeline_version,
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
        store.set("upload_target", serde_json::json!(target));
    } else {
        store.delete("upload_target");
    }

    store.set("skip_pii_check", serde_json::json!(settings.skip_pii_check));
    store.set("pipeline_version", serde_json::json!(settings.pipeline_version));

    // API key goes to keyring
    if let Some(key) = &settings.api_key {
        if !key.is_empty() {
            crate::network::auth::keyring_save("openai-key", key).map_err(|e| e.to_string())?;
        } else {
            let _ = crate::network::auth::keyring_clear("openai-key");
        }
    } else {
        let _ = crate::network::auth::keyring_clear("openai-key");
    }

    Ok(())
}

#[tauri::command]
pub async fn get_webapp_url(app: tauri::AppHandle) -> Result<String, String> {
    if cfg!(debug_assertions) {
        let target = app
            .store(STORE_FILENAME)
            .ok()
            .and_then(|store| {
                store
                    .get("upload_target")
                    .and_then(|v| v.as_str().map(String::from))
            });
        Ok(crate::config::webapp_url_for_target(target.as_deref()).to_string())
    } else {
        Ok(crate::config::WEBAPP_URL_PROD.to_string())
    }
}
