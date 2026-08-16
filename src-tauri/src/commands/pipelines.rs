use tauri::State;
use tauri_plugin_store::StoreExt;

use crate::commands::auth::{get_api_base, SessionCache};
use crate::network::{auth as net_auth, pipelines as net_pipelines};

const STORE_FILENAME: &str = "settings.json";

/// Store key holding the user's last pipeline choice. Deliberately not part of
/// `AppSettings`: that model backs the Settings window, which is gated per org
/// by `advanced_settings`. The pipeline selector is a user control and must be
/// available to every org, so it does not travel with the engineering knobs.
const SELECTED_KEY: &str = "pipeline_id";

/// Fetch the pipeline catalogue.
///
/// Never fails the caller: an unreachable server returns the last good
/// catalogue, or an empty list if there has never been one. A failed catalogue
/// fetch must not surface as an error over a recording that succeeded.
#[tauri::command]
pub async fn get_pipelines(
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
) -> Result<Vec<net_pipelines::Pipeline>, String> {
    let api_base = get_api_base(&app);

    let access_token = match net_auth::refresh_session(api_base).await {
        Ok(Some(auth)) => {
            let token = auth.access_token.clone();
            *session.access_token.lock().unwrap() = Some(auth.access_token);
            token
        }
        _ => match session.access_token.lock().unwrap().clone() {
            Some(token) => token,
            None => return Ok(last_good(&app)),
        },
    };

    match net_pipelines::fetch_pipelines(&access_token, Some(api_base)).await {
        Ok(pipelines) => {
            save_last_good(&app, &pipelines);
            Ok(pipelines)
        }
        Err(e) => {
            log::warn!("Pipeline catalogue fetch failed: {} -- serving last good", e);
            Ok(last_good(&app))
        }
    }
}

/// Remember the user's choice so it can be preselected next time. An empty
/// string clears it.
#[tauri::command]
pub async fn set_selected_pipeline(app: tauri::AppHandle, pipeline_id: String) -> Result<(), String> {
    let store = app.store(STORE_FILENAME).map_err(|e| e.to_string())?;
    if pipeline_id.is_empty() {
        store.delete(SELECTED_KEY);
    } else {
        store.set(SELECTED_KEY, serde_json::json!(pipeline_id));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_selected_pipeline(app: tauri::AppHandle) -> Result<String, String> {
    Ok(selected_pipeline_id(&app).unwrap_or_default())
}

/// The stored selection, for `run_generation` to put on the upload.
pub fn selected_pipeline_id(app: &tauri::AppHandle) -> Option<String> {
    app.store(STORE_FILENAME)
        .ok()?
        .get(SELECTED_KEY)
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.is_empty())
}

const LAST_GOOD_KEY: &str = "pipelines_last_good";

fn last_good(app: &tauri::AppHandle) -> Vec<net_pipelines::Pipeline> {
    app.store(STORE_FILENAME)
        .ok()
        .and_then(|store| store.get(LAST_GOOD_KEY))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn save_last_good(app: &tauri::AppHandle, pipelines: &[net_pipelines::Pipeline]) {
    if let Ok(store) = app.store(STORE_FILENAME) {
        store.set(LAST_GOOD_KEY, serde_json::json!(pipelines));
    }
}
