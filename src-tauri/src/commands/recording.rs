use chrono::Local;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::capture::audio::AudioHandle;
use crate::capture::input_hooks;
use crate::capture::screenshot;
use crate::output::pending;
use crate::state::{AppState, RecordingSession, RecordingStatus};

fn get_output_dir(app: &tauri::AppHandle) -> PathBuf {
    let name = app.config().product_name.as_deref().unwrap_or("sop-sorcery");
    let default_dir = dirs_next::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(format!("{} Workflows", name));

    if let Ok(store) = app.store("settings.json") {
        if let Some(val) = store.get("output_dir") {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    return PathBuf::from(s);
                }
            }
        }
    }

    default_dir
}

#[tauri::command]
pub async fn start_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut status = state.recording_status.lock().unwrap();
    if *status == RecordingStatus::Recording {
        return Err("Already recording".into());
    }

    // Create output directory
    let timestamp = Local::now().format("%Y-%m-%d %H-%M-%S").to_string();
    let guide_title = format!("SOP {}", timestamp);
    let base_dir = get_output_dir(&app);
    let output_dir = base_dir.join(&guide_title);
    let screenshots_dir = output_dir.join("screenshots");

    fs::create_dir_all(&screenshots_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    // Apply hide-from-screenshots setting
    let hide = if let Ok(store) = app.store("settings.json") {
        store
            .get("hide_from_screenshots")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    } else {
        true
    };
    if let Err(e) = crate::commands::window::set_display_affinity(app.clone(), hide) {
        log::warn!("Failed to set display affinity: {}", e);
    }

    // Start audio recording
    let audio_path = output_dir.join("recording.wav");
    let audio_handle =
        AudioHandle::start(&audio_path).map_err(|e| format!("Audio start failed: {}", e))?;

    // Shared step counter and in-flight tracker
    let step_counter = Arc::new(AtomicU32::new(0));
    let in_flight = Arc::new(AtomicU32::new(0));

    // Reset and reuse the shared stop flag from AppState -- lives outside the
    // session mutex so stop_recording can set it instantly (no race window).
    let stop_flag = state.capture_stop_flag.clone();
    stop_flag.store(false, Ordering::SeqCst);

    // Get recorder window HWND so clicks on it are ignored (works even if moved)
    let exclude_hwnd = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);

    // Start input hooks -- screenshots are captured immediately in the callback
    let counter_clone = step_counter.clone();
    let in_flight_clone = in_flight.clone();
    let stop_clone = stop_flag.clone();
    let screenshots_dir_clone = screenshots_dir.clone();
    let app_clone = app.clone();

    let hook_handle = input_hooks::start_listener_with_callback(exclude_hwnd, move |event| {
        if stop_clone.load(Ordering::SeqCst) {
            return;
        }

        let click_pos = match &event {
            input_hooks::CaptureEvent::MouseClick { .. } => input_hooks::get_cursor_position(),
            input_hooks::CaptureEvent::EnterKey => None,
        };

        let step_num = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;

        // Spawn capture work so the rdev thread isn't blocked
        let dir = screenshots_dir_clone.clone();
        let app = app_clone.clone();
        let flight = in_flight_clone.clone();
        flight.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            match screenshot::capture_and_save(&dir, step_num, click_pos) {
                Ok(_filename) => {
                    let _ = app.emit("recording:step_captured", step_num);
                }
                Err(e) => {
                    log::error!("Screenshot capture failed: {}", e);
                }
            }
            flight.fetch_sub(1, Ordering::SeqCst);
        });
    });

    // Create session
    let session = RecordingSession {
        output_dir: output_dir.clone(),
        screenshots_dir,
        guide_title,
        steps: Vec::new(),
        audio_handle: Some(audio_handle),
        input_hook: Some(hook_handle),
        stop_flag: Some(stop_flag),
        in_flight: Some(in_flight),
    };

    *state.current_session.lock().unwrap() = Some(session);
    *status = RecordingStatus::Recording;

    log::info!("Recording started: {}", output_dir.display());
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    // Set stop flag IMMEDIATELY -- before locking the session mutex.
    // This prevents the race where rdev fires the stop-button click
    // before the session lock is acquired.
    state.capture_stop_flag.store(true, Ordering::SeqCst);

    {
        let mut status = state.recording_status.lock().unwrap();
        if *status != RecordingStatus::Recording {
            return Err("Not recording".into());
        }
        *status = RecordingStatus::Processing;
    }

    // Extract everything from the session in one scoped block
    let (in_flight_counter, audio, output_dir_path, guide_title) = {
        let mut session_guard = state.current_session.lock().unwrap();
        let session = session_guard.as_mut().ok_or("No active session")?;

        // Clear session's copy of the flag (already set above)
        session.stop_flag.take();

        // Stop input hooks
        if let Some(hook) = session.input_hook.take() {
            hook.stop();
        }

        (
            session.in_flight.take(),
            session.audio_handle.take(),
            session.output_dir.clone(),
            session.guide_title.clone(),
        )
    }; // MutexGuard dropped here

    // Restore window visibility
    let _ = crate::commands::window::set_display_affinity(app, false);

    // Stop audio immediately
    if let Some(audio) = audio {
        audio.stop().map_err(|e| format!("Audio stop failed: {}", e))?;
    }

    // Store in-flight counter in app state so generate can wait for it
    *state.in_flight_captures.lock().unwrap() = in_flight_counter;

    // Write pending.json
    pending::write_pending(&output_dir_path, &guide_title)
        .map_err(|e| format!("Failed to write pending marker: {}", e))?;

    let output_dir = output_dir_path.to_string_lossy().to_string();
    log::info!("Recording stopped. Output: {}", output_dir);

    Ok(output_dir)
}
