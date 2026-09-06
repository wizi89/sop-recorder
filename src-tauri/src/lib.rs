pub mod capture;
pub mod commands;
pub mod config;
pub mod error_reports;
pub mod network;
pub mod output;
pub mod runtime_config;
pub mod state;
use commands::{auth, generate, permissions, pipelines, quota, recording, settings, window};

/// The saved `error_reports` mode, read straight from the settings file.
///
/// The panic hook is installed before the Tauri builder runs (design D5), so
/// the store plugin is not up yet and `get_settings` cannot be called. This
/// reads the same JSON document the store writes. A file that is missing or
/// unreadable yields `None`, which resolves to the default, `ask`.
fn stored_error_report_mode() -> Option<String> {
    let path = dirs_next::data_dir()?
        .join("com.cogniclone.recorder")
        .join("settings.json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()?
        .get("error_reports")?
        .as_str()
        .map(str::to_string)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Before anything reads the runtime overrides, and before logging is set
    // up: a bad flag should fail on the terminal that typed it rather than in a
    // log file the user has not opened.
    let args: Vec<String> = std::env::args().collect();
    let backend = match runtime_config::parse_backend_flag(&args) {
        Ok(backend) => backend,
        Err(message) => {
            eprintln!("cogniclone: {}", message);
            std::process::exit(2);
        }
    };
    runtime_config::init(backend);

    // Configure log targets:
    // - Dev mode: stdout + cogniclone .tmp/logs/ (side-by-side with server logs)
    // - Release mode: AppData/Roaming/{identifier}/logs/ (next to settings)
    let reqwest_connect_level = if cfg!(debug_assertions) {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };
    let mut log_builder = tauri_plugin_log::Builder::default()
        // The suppression log IS the audit trail: D4 requires every dropped
        // input event to leave a trace precisely so the rule can be checked
        // afterwards. At the plugin's defaults (40 KB, discard the rotated
        // file) that trail destroys itself: one Enter held for five seconds
        // emits ~80 auto-repeat lines in under three seconds, which rotated a
        // real recording's first 35 seconds out of existence and made an audit
        // report "no clicks were suppressed" for a recording in which three
        // were. A false negative from the evidence channel is worse than no
        // evidence channel.
        .max_file_size(5_000_000)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
        .level(log::LevelFilter::Info)
        .level_for("keyring", log::LevelFilter::Warn)
        .level_for("reqwest::connect", reqwest_connect_level)
        .level_for("reqwest::retry", log::LevelFilter::Warn)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("tauri_plugin_updater", log::LevelFilter::Warn);
    // The ring buffer an error report's `log_tail` is read from (design D4).
    // `target()` appends to the plugin's defaults, so the release build keeps
    // writing its log file to the platform log directory; the dev path below
    // calls `targets()`, which replaces them, and so has to list it again.
    log_builder = log_builder.target(error_reports::ring_log_target());
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
                        error_reports::ring_log_target(),
                    ]);
            }
        }
    }

    // Before the builder, so a panic during startup is reported too (design
    // D5). Nothing here touches the store or the network: the hook writes a
    // file and calls the previous hook, and everything else happens later,
    // from the webview, off the back of that file.
    // On the terminal, not through `log`: the logger plugin is not installed
    // until the builder below runs, so a `log::` call here reaches nobody. The
    // same line is written to the log inside `setup`, where it does.
    if let Some(backend) = backend {
        eprintln!(
            "cogniclone: backend overridden on the command line: --{} ({})",
            backend.as_str(),
            backend.api_url(),
        );
    }

    if let Some(dir) = error_reports::reports_dir() {
        error_reports::set_active_reports_dir(dir);
    }
    error_reports::set_mode(error_reports::resolve_mode(
        stored_error_report_mode().as_deref(),
        runtime_config::runtime().error_reports_forced_off(),
    ));
    error_reports::set_phase(error_reports::Phase::Startup);
    error_reports::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(log_builder.build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Now that the logger exists. Into the log file and the ring buffer
            // an error report carries, so a report from a run like this says
            // which backend it was against -- the 2026-09-03 test was run on
            // production by someone who believed it was staging.
            if let Some(backend) = backend {
                log::warn!(
                    "Backend overridden on the command line: --{} ({})",
                    backend.as_str(),
                    backend.api_url(),
                );
            }
            error_reports::set_app_handle(app.handle().clone());
            network::auth::migrate_keyring();
            settings::AppSettings::initialize(app.handle());
            // The settings subset a report carries, and the output directory
            // the log scrubber needs, held where the panic hook can read them
            // without a lock on the store.
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(current) = settings::get_settings(handle).await {
                        settings::publish_error_report_context(&current);
                    }
                });
            }
            error_reports::set_phase(error_reports::Phase::Idle);
            // Built here, not on demand: the bar can only join another app's
            // fullscreen Space if it is created under an accessory activation
            // policy, and that is a property of the window from birth.
            if let Err(e) = commands::window::create_recording_bar(app.handle()) {
                log::error!("Could not build the recording bar: {}", e);
            }
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
            settings::is_updater_enabled,
            settings::are_error_reports_forced_off,
            commands::error_reports::list_error_reports,
            commands::error_reports::read_error_report,
            commands::error_reports::create_error_report,
            commands::error_reports::decide_error_report,
            commands::error_reports::error_report_path,
            commands::error_reports::submit_error_reports,
            commands::error_reports::set_error_report_phase,
            commands::error_reports::debug_trigger_failure,
            recording::start_recording,
            recording::stop_recording,
            recording::delete_last_screenshot,
            recording::list_session_screenshots,
            recording::read_screenshot_bytes,
            generate::run_generation,
            quota::get_quota,
            pipelines::get_pipelines,
            pipelines::get_selected_pipeline,
            pipelines::set_selected_pipeline,
            permissions::get_microphone_permission_state,
            permissions::get_screen_recording_permission_state,
            permissions::get_accessibility_permission_state,
            permissions::open_privacy_settings,
            permissions::request_permission,
            window::set_display_affinity,
            window::set_recorder_region,
            window::restart_app,
            window::get_work_area,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
