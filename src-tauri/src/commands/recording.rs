use chrono::Local;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tauri::{Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::capture::audio::AudioHandle;
use crate::capture::input_hooks;
use crate::capture::screenshot;
use crate::output::pending;
use crate::state::{AppState, CapturedStep, RecordingSession, RecordingStatus};

fn get_output_dir(app: &tauri::AppHandle) -> PathBuf {
    let default_dir = dirs_next::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Wizimate Workflows");

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

    // Start audio recording
    let audio_path = output_dir.join("recording.wav");
    let audio_handle =
        AudioHandle::start(&audio_path).map_err(|e| format!("Audio start failed: {}", e))?;

    // Start input hooks (mouse + keyboard)
    let (hook_handle, event_rx) = input_hooks::start_listener(None);

    // Share recording status and steps for the capture thread
    let recording_status = Arc::new(std::sync::Mutex::new(RecordingStatus::Recording));
    let steps = Arc::new(std::sync::Mutex::new(Vec::<CapturedStep>::new()));

    let status_clone = recording_status.clone();
    let steps_clone = steps.clone();
    let screenshots_dir_clone = screenshots_dir.clone();
    let app_clone = app.clone();

    thread::spawn(move || {
        let mut step_count: u32 = 0;

        while let Ok(event) = event_rx.recv() {
            {
                let st = status_clone.lock().unwrap();
                if *st != RecordingStatus::Recording {
                    break;
                }
            }

            step_count += 1;

            let click_pos = match &event {
                input_hooks::CaptureEvent::MouseClick { .. } => {
                    input_hooks::get_cursor_position()
                }
                input_hooks::CaptureEvent::EnterKey => None,
            };

            match screenshot::capture_and_save(
                &screenshots_dir_clone,
                step_count,
                click_pos,
            ) {
                Ok(filename) => {
                    let step = CapturedStep {
                        order_id: step_count,
                        filename,
                        click_x: click_pos.map(|(x, _)| x),
                        click_y: click_pos.map(|(_, y)| y),
                    };

                    steps_clone.lock().unwrap().push(step);
                    let _ = app_clone.emit("recording:step_captured", step_count);
                }
                Err(e) => {
                    log::error!("Screenshot capture failed: {}", e);
                }
            }
        }
    });

    // Create session
    let session = RecordingSession {
        output_dir: output_dir.clone(),
        screenshots_dir,
        guide_title,
        steps: Vec::new(),
        audio_handle: Some(audio_handle),
        input_hook: Some(hook_handle),
    };

    *state.current_session.lock().unwrap() = Some(session);
    *status = RecordingStatus::Recording;

    log::info!("Recording started: {}", output_dir.display());
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<String, String> {
    let mut status = state.recording_status.lock().unwrap();
    if *status != RecordingStatus::Recording {
        return Err("Not recording".into());
    }

    *status = RecordingStatus::Processing;
    drop(status); // Release lock before potentially blocking operations

    let mut session_guard = state.current_session.lock().unwrap();
    let session = session_guard
        .as_mut()
        .ok_or("No active session")?;

    // Stop input hooks first
    if let Some(hook) = session.input_hook.take() {
        hook.stop();
    }

    // Stop audio recording
    if let Some(audio) = session.audio_handle.take() {
        audio.stop().map_err(|e| format!("Audio stop failed: {}", e))?;
    }

    // Write pending.json
    pending::write_pending(&session.output_dir, &session.guide_title)
        .map_err(|e| format!("Failed to write pending marker: {}", e))?;

    let output_dir = session.output_dir.to_string_lossy().to_string();
    log::info!("Recording stopped. Output: {}", output_dir);

    Ok(output_dir)
}
