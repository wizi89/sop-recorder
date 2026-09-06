use chrono::Local;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
#[cfg(windows)]
use tauri::Manager;
use tauri::{Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::capture::audio::AudioHandle;
use crate::capture::input_hooks;
use crate::capture::permits::CapturePermits;
use crate::capture::screenshot;
use crate::output::pending;
use crate::output::step_meta::{self, StepMeta};
use crate::state::{AppState, RecordingSession, RecordingStatus};

fn get_output_dir(app: &tauri::AppHandle) -> PathBuf {
    let name = app.config().product_name.as_deref().unwrap_or("cogniclone");
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
        return Err("Eine Aufnahme läuft bereits.".into());
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

    // Capture the reference instant BEFORE starting audio so screenshot
    // timestamps reference the same start point as the audio timeline.
    // Audio capture initialization adds a small constant offset (a few ms)
    // which is well within the per-step transcript buffer downstream.
    let recording_start = Instant::now();

    // Start audio recording
    let audio_path = output_dir.join("recording.wav");
    let audio_handle =
        AudioHandle::start(&audio_path).map_err(|e| format!("Audio start failed: {}", e))?;

    // Reset the shared stop flag BEFORE spawning any background tasks that
    // check it, so a stale value from a previous session cannot terminate
    // them prematurely.
    state.capture_stop_flag.store(false, Ordering::SeqCst);

    // Long enough that a pause before the user starts speaking is not mistaken
    // for a dead microphone, short enough that they are told while the
    // recording is still worth restarting.
    const SILENCE_WARNING_AFTER_SECS: u32 = 5;

    // Spawn the audio-level poller: at ~10 Hz, read the latest peak level
    // from the shared atomic and emit a Tauri event so the frontend can
    // render a live VU meter. Exits when the capture stop flag is set.
    let audio_level_atomic = audio_handle.audio_level.clone();
    let vu_stop_flag = state.capture_stop_flag.clone();
    let vu_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

        // A microphone the user denied does not make the stream fail: macOS
        // vends silent samples, so the recording completes and the SOP is
        // generated with an empty narration and no warning anywhere. Exact
        // zeroes are what distinguishes that from a quiet room -- a live input
        // always carries some noise floor, however small.
        let mut silent_ticks: u32 = 0;
        let mut silence_reported = false;
        const SILENCE_TICKS_BEFORE_WARNING: u32 = SILENCE_WARNING_AFTER_SECS * 10;

        loop {
            interval.tick().await;
            if vu_stop_flag.load(Ordering::SeqCst) {
                break;
            }
            let bits = audio_level_atomic.load(Ordering::Relaxed);
            let level = f32::from_bits(bits);
            let _ = vu_app.emit("recording:audio_level", level);

            if level == 0.0 {
                silent_ticks += 1;
                if silent_ticks >= SILENCE_TICKS_BEFORE_WARNING && !silence_reported {
                    silence_reported = true;
                    log::warn!(
                        "Audio has been exactly silent for {}s -- the microphone is most \
                         likely denied, and this recording will have no narration",
                        SILENCE_WARNING_AFTER_SECS
                    );
                    let _ = vu_app.emit("recording:audio_silent", ());
                }
            } else {
                // One real sample is enough to settle the question for good:
                // a granted microphone that happens to go quiet later is not
                // the failure this warns about.
                silent_ticks = 0;
            }
        }
    });

    // Shared step counter and in-flight tracker
    let step_counter = Arc::new(AtomicU32::new(0));
    let in_flight = Arc::new(AtomicU32::new(0));
    let failed_captures = Arc::new(AtomicU32::new(0));
    // Bounds how many full-screen buffers are resident at once. Per recording
    // rather than global: nothing captures between recordings.
    let capture_permits = Arc::new(CapturePermits::default());

    // Clone the shared stop flag for the input-hook thread.
    let stop_flag = state.capture_stop_flag.clone();

    // Get recorder window HWND so clicks on it are ignored (Windows only).
    //
    // The bar, not the main window: the bar is what is on screen while a
    // recording runs, so it is the window the user's Stop click lands on. This
    // read "main" until the bar became a window of its own, and pointing it at
    // the wrong window silently turns every press of Stop into a captured step.
    #[cfg(windows)]
    let exclude_hwnd = app
        .get_webview_window(crate::commands::window::BAR_LABEL)
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize);

    #[cfg(not(windows))]
    let exclude_hwnd = None;

    // Start input hooks -- screenshots are captured immediately in the callback
    let counter_clone = step_counter.clone();
    let in_flight_clone = in_flight.clone();
    let failed_clone = failed_captures.clone();
    let permits_clone = capture_permits.clone();
    let stop_clone = stop_flag.clone();
    let screenshots_dir_clone = screenshots_dir.clone();
    let app_clone = app.clone();

    let hook_handle = input_hooks::start_listener_with_callback(exclude_hwnd, move |event| {
        if stop_clone.load(Ordering::SeqCst) {
            return;
        }

        // The hook carries the position it made the suppression decision with,
        // so the overlay is drawn at the same point that decision used. Polling
        // the cursor again here would sample a different moment.
        let (click_pos, trigger) = match &event {
            input_hooks::CaptureEvent::MouseClick { pos } => (*pos, "mouse_click"),
            input_hooks::CaptureEvent::EnterKey => (None, "enter_key"),
        };

        // Stamp the trigger time at the moment the input event fires, before
        // the screenshot capture is spawned. This is the time we want
        // attached to the step, not the (variable) write-completion time.
        let timestamp_seconds = recording_start.elapsed().as_secs_f64();
        let step_num = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;

        // Spawn capture work so the rdev thread isn't blocked
        let dir = screenshots_dir_clone.clone();
        let app = app_clone.clone();
        let flight = in_flight_clone.clone();
        let failed = failed_clone.clone();
        let permits = permits_clone.clone();
        // Incremented before the spawn, so the counter covers a capture that is
        // still queued for a permit as well as one already running. That is what
        // `stop_recording` and the generate path wait on, and a queued capture
        // is exactly the kind they must not race past.
        flight.fetch_add(1, Ordering::SeqCst);
        std::thread::spawn(move || {
            // Held for the capture and the sidecar write, released on the way
            // out of this scope however the capture ends.
            let _permit = permits.acquire();
            match screenshot::capture_and_save(&dir, step_num, click_pos) {
                Ok(marker_box) => {
                    // Write the sidecar AFTER the PNG has been written so a
                    // crash between PNG and JSON leaves an unpaired PNG that
                    // the upload path can detect (and either skip or fail
                    // loudly), rather than a JSON pointing at a missing PNG.
                    let meta = StepMeta {
                        order: step_num,
                        timestamp_seconds,
                        click_x: click_pos.map(|(x, _)| x),
                        click_y: click_pos.map(|(_, y)| y),
                        trigger: trigger.to_string(),
                        marker_box,
                    };
                    if let Err(e) = step_meta::write_sidecar(&dir, &meta) {
                        log::error!(
                            "Step sidecar write failed for step {}: {}",
                            step_num, e
                        );
                    }
                    let _ = app.emit("recording:step_captured", step_num);
                }
                Err(e) => {
                    // Counted, not just logged. This is the path that turned 21
                    // clicks into a one-step guide on 2026-09-03 while the app
                    // reported nothing at all.
                    let total = failed.fetch_add(1, Ordering::SeqCst) + 1;
                    log::error!(
                        "Screenshot capture failed for step {} ({} failed so far): {}",
                        step_num, total, e
                    );
                    let _ = app.emit("recording:step_failed", total);
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
        audio_handle: Some(audio_handle),
        input_hook: Some(hook_handle),
        stop_flag: Some(stop_flag),
        in_flight: Some(in_flight),
        step_counter: Some(step_counter),
        failed_captures: Some(failed_captures),
    };

    *state.current_session.lock().unwrap() = Some(session);
    *status = RecordingStatus::Recording;
    // What a panic report says the app was doing (design D3). Set here rather
    // than read out of this mutex at panic time: the panic hook must not take
    // a lock it may be racing or that may already be poisoned.
    crate::error_reports::set_phase(crate::error_reports::Phase::Recording);

    let _ = app.emit("recording:started", ());
    log::info!("Recording started: {}", output_dir.display());
    Ok(())
}

/// What a stopped recording leaves behind: where it was written, and how much
/// of it did not survive. The count travels with the folder rather than only as
/// an event, so the review screen cannot show a complete-looking recording
/// because it missed a notification.
#[derive(serde::Serialize)]
pub struct StoppedRecording {
    pub output_dir: String,
    pub failed_captures: u32,
}

#[tauri::command]
pub async fn stop_recording(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StoppedRecording, String> {
    // Set stop flag IMMEDIATELY -- before locking the session mutex.
    // This prevents the race where rdev fires the stop-button click
    // before the session lock is acquired.
    state.capture_stop_flag.store(true, Ordering::SeqCst);

    {
        let mut status = state.recording_status.lock().unwrap();
        if *status != RecordingStatus::Recording {
            return Err("Keine aktive Aufnahme.".into());
        }
        *status = RecordingStatus::Processing;
        crate::error_reports::set_phase(crate::error_reports::Phase::Processing);
    }

    // Extract everything from the session in one scoped block
    let (in_flight_counter, failed_counter, audio, output_dir_path, guide_title) = {
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
            session.failed_captures.take(),
            session.audio_handle.take(),
            session.output_dir.clone(),
            session.guide_title.clone(),
        )
    }; // MutexGuard dropped here

    // Restore window visibility
    let _ = crate::commands::window::set_display_affinity(app.clone(), false);

    // Stop audio immediately
    if let Some(audio) = audio {
        audio.stop().map_err(|e| format!("Audio stop failed: {}", e))?;
    }

    // Wait for any in-flight screenshot captures to finish writing to disk
    // BEFORE returning, so the review screen opens onto a stable filesystem
    // state. On a fast machine this is near-instant; on a slow one the last
    // click's capture + resize can take a second or two. Cap at 15s to avoid
    // a truly stuck stop.
    if let Some(counter) = in_flight_counter.as_ref() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut waited = false;
        while counter.load(Ordering::SeqCst) > 0 {
            if std::time::Instant::now() > deadline {
                log::warn!("stop_recording: in-flight captures did not finish within 15s");
                break;
            }
            if !waited {
                log::info!(
                    "stop_recording: waiting for {} in-flight capture(s) to finish",
                    counter.load(Ordering::SeqCst)
                );
                waited = true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    // Store in-flight counter in app state (kept as defense in depth for the
    // generate path; should always be 0 here after the wait above).
    *state.in_flight_captures.lock().unwrap() = in_flight_counter;

    // Write pending.json. Per-step metadata lives in step_NN.json sidecars
    // alongside each PNG, so pending.json only needs the guide title.
    pending::write_pending(&output_dir_path, &guide_title)
        .map_err(|e| format!("Failed to write pending marker: {}", e))?;

    let output_dir = output_dir_path.to_string_lossy().to_string();
    // Read after the in-flight wait above, so a capture that failed while the
    // queue drained is included rather than raced past.
    let failed_captures = failed_counter
        .as_ref()
        .map(|c| c.load(Ordering::SeqCst))
        .unwrap_or(0);
    let _ = app.emit("recording:stopped", ());
    if failed_captures > 0 {
        log::warn!(
            "Recording stopped with {} failed capture(s). Output: {}",
            failed_captures, output_dir
        );
    } else {
        log::info!("Recording stopped. Output: {}", output_dir);
    }

    Ok(StoppedRecording { output_dir, failed_captures })
}

/// Read the raw bytes of a screenshot file. Returns a byte array the
/// frontend can wrap in a Blob to display as a thumbnail without needing
/// the Tauri asset protocol to be configured.
#[tauri::command]
pub async fn read_screenshot_bytes(path: String) -> Result<Vec<u8>, String> {
    fs::read(&path).map_err(|e| format!("Failed to read {}: {}", path, e))
}

/// List the captured screenshot files in a session output directory, in
/// capture order. Returns absolute file paths as strings so the React side
/// can render them via `read_screenshot_bytes`.
///
/// This works without an active session: it just scans the given
/// `{output_dir}/screenshots/` folder, so it can be called during the
/// review screen even after `stop_recording` has finalized the session.
#[tauri::command]
pub async fn list_session_screenshots(output_dir: String) -> Result<Vec<String>, String> {
    let screenshots_dir = PathBuf::from(&output_dir).join("screenshots");
    if !screenshots_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(u32, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&screenshots_dir)
        .map_err(|e| format!("Failed to read screenshots dir: {}", e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Parse "step_NN.png" -> NN
            if let Some(stripped) = name
                .strip_prefix("step_")
                .and_then(|s| s.strip_suffix(".png"))
            {
                if let Ok(n) = stripped.parse::<u32>() {
                    entries.push((n, path));
                }
            }
        }
    }

    entries.sort_by_key(|(n, _)| *n);
    Ok(entries
        .into_iter()
        .map(|(_, p)| p.to_string_lossy().to_string())
        .collect())
}

/// Delete the most recently captured screenshot from the active session.
///
/// Returns the new screenshot count after deletion. Emits a
/// `recording:step_deleted` Tauri event with the new count so the React
/// capture counter hook can decrement the UI.
///
/// Errors if:
/// - there is no active recording session
/// - the step counter is at 0 (nothing to delete)
/// - a screenshot capture is currently in-flight (racy, reject)
#[tauri::command]
pub async fn delete_last_screenshot(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let session_guard = state.current_session.lock().unwrap();
    let session = session_guard
        .as_ref()
        .ok_or("Keine aktive Aufnahme.")?;

    // Refuse if a capture is currently being written to disk.
    if let Some(flight) = session.in_flight.as_ref() {
        if flight.load(Ordering::SeqCst) > 0 {
            return Err("Aufnahme läuft, bitte einen Moment warten.".into());
        }
    }

    let counter = session
        .step_counter
        .as_ref()
        .ok_or("Sitzung hat keinen Schritt-Zähler.")?;
    let current = counter.load(Ordering::SeqCst);
    if current == 0 {
        return Err("Keine Aufnahmen zum Rückgängigmachen.".into());
    }

    // Decrement first, then delete the file. If the delete fails, roll back
    // the counter so the next capture lands on the correct step number.
    let new_count = counter.fetch_sub(1, Ordering::SeqCst) - 1;
    let filename = format!("step_{:02}.png", current);
    let path = session.screenshots_dir.join(&filename);

    match fs::remove_file(&path) {
        Ok(()) => {
            log::info!("Undo: deleted {}", path.display());
            // Remove the matching sidecar so the upload path doesn't see a
            // dangling JSON pointing at a missing PNG. If the sidecar write
            // hadn't completed yet (rare race), `delete_sidecar` is a no-op.
            if let Err(e) = step_meta::delete_sidecar(&session.screenshots_dir, current) {
                log::warn!("Undo: failed to delete sidecar for step {}: {}", current, e);
            }
            let _ = app.emit("recording:step_deleted", new_count);
            Ok(new_count)
        }
        Err(e) => {
            // Roll back the counter so the next capture still numbers correctly.
            counter.fetch_add(1, Ordering::SeqCst);
            Err(format!("Failed to delete {}: {}", path.display(), e))
        }
    }
}
