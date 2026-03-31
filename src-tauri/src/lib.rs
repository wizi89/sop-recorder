pub mod capture;
pub mod commands;
pub mod config;
pub mod network;
pub mod output;
pub mod state;
use commands::{auth, generate, recording, settings, window};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Configure log targets:
    // - Dev mode: stdout + sop-sorcery .tmp/logs/ (side-by-side with server logs)
    // - Release mode: AppData/Roaming/{identifier}/logs/ (next to settings)
    let mut log_builder = tauri_plugin_log::Builder::default();
    if cfg!(debug_assertions) {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let log_dir = manifest_dir
            .parent() // sop-recorder
            .and_then(|p| p.parent()) // parent of both repos
            .map(|p| p.join("9_sop-sorcery").join(".tmp").join("logs"));
        if let Some(dir) = log_dir {
            if dir.parent().map_or(false, |p| p.exists()) {
                let _ = std::fs::create_dir_all(&dir);
                log_builder = log_builder
                    .targets([
                        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                        tauri_plugin_log::Target::new(
                            tauri_plugin_log::TargetKind::Folder { path: dir, file_name: Some("recorder".into()) },
                        ),
                    ]);
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(log_builder.build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            settings::AppSettings::initialize(app.handle());
            Ok(())
        })
        .manage(state::AppState::default())
        .manage(auth::SessionCache::default())
        .invoke_handler(tauri::generate_handler![
            auth::login,
            auth::logout,
            auth::refresh_session,
            auth::get_session_state,
            settings::get_settings,
            settings::save_settings,
            settings::get_webapp_url,
            recording::start_recording,
            recording::stop_recording,
            generate::run_generation,
            window::set_display_affinity,
            window::get_work_area,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
