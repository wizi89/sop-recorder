pub mod capture;
pub mod commands;
pub mod network;
pub mod output;
pub mod state;
mod tray;

use commands::{auth, generate, recording, settings, window};
use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .manage(auth::SessionCache::default())
        .setup(|app| {
            tray::create_tray(app)?;

            // Check for updates on startup (non-blocking)
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match tauri_plugin_updater::UpdaterExt::updater(&handle) {
                    Ok(updater) => {
                        match updater.check().await {
                            Ok(Some(update)) => {
                                log::info!("Update available: {}", update.version);
                                let _ = handle.emit("update:available", update.version.clone());
                            }
                            Ok(None) => {
                                log::info!("App is up to date");
                            }
                            Err(e) => {
                                log::warn!("Update check failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Updater not available: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::login,
            auth::logout,
            auth::refresh_session,
            auth::get_session_state,
            settings::get_settings,
            settings::save_settings,
            recording::start_recording,
            recording::stop_recording,
            generate::run_generation,
            window::set_display_affinity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
