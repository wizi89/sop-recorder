use serde::{Deserialize, Serialize};
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
}

impl Default for AppSettings {
    fn default() -> Self {
        let docs = dirs_next::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Wizimate Workflows");
        let app_data = dirs_next::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("com.wizimate.recorder")
            .join("logs");

        Self {
            output_dir: docs.to_string_lossy().to_string(),
            logs_dir: app_data.to_string_lossy().to_string(),
            hide_from_screenshots: true,
            api_key: None,
            upload_target: None,
            skip_pii_check: false,
        }
    }
}

#[tauri::command]
pub async fn get_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let store = app.store(STORE_FILENAME).map_err(|e| e.to_string())?;
    let defaults = AppSettings::default();

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

    // API key is stored in keyring, not in the store
    let api_key = crate::network::auth::keyring_load("openai-key").ok().flatten();

    Ok(AppSettings {
        output_dir,
        logs_dir,
        hide_from_screenshots,
        api_key,
        upload_target,
        skip_pii_check,
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
