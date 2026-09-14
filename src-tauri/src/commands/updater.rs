//! Update checks, on the channel the user chose.
//!
//! The frontend used to call the plugin's own JS `check()`, which resolves its
//! endpoint from `tauri.conf.json` and is therefore fixed at build time --
//! `CheckOptions` carries headers, timeout, proxy and target, but no
//! endpoints. Only `UpdaterBuilder::endpoints` can vary it, and that is Rust
//! only, which is why these two commands exist at all.

use tauri_plugin_updater::{Update, UpdaterExt};

const STORE_FILENAME: &str = "settings.json";

/// The endpoint for the channel the user is on. An unreadable or unset
/// setting means stable: opting in to betas is a deliberate act, so every
/// failure here has to land on the channel the user did not have to choose.
fn endpoint(app: &tauri::AppHandle) -> &'static str {
    let beta = tauri_plugin_store::StoreExt::store(app, STORE_FILENAME)
        .ok()
        .and_then(|store| store.get("beta_updates").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    crate::config::update_endpoint(beta)
}

async fn pending(app: &tauri::AppHandle) -> Result<Option<Update>, String> {
    let url = url::Url::parse(endpoint(app)).map_err(|e| e.to_string())?;
    app.updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())
}

/// The version on offer, or `None` when the installed build is current.
#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(pending(&app).await?.map(|update| update.version))
}

/// On Windows NSIS this closes the app, installs, and relaunches.
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    // Re-checked rather than held between the two calls. One extra request
    // costs less than an `Update` parked in managed state, which would outlive
    // a channel the user switched in between and install from the old one.
    let Some(update) = pending(&app).await? else {
        return Ok(());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())
}
