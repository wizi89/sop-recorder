//! Stopping a recording.

use super::*;

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
    let result = stop_recording_inner(app, state.clone()).await;
    if result.is_err() {
        // A stop that failed part-way, after the status moved to Processing,
        // would otherwise leave it there and every later error report
        // labelled with a phase the app is no longer in.
        *state
            .recording_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = RecordingStatus::Idle;
        crate::error_reports::set_phase(crate::error_reports::Phase::Idle);
    }
    result
}

pub(super) async fn stop_recording_inner(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<StoppedRecording, String> {
    // Set stop flag IMMEDIATELY -- before locking the session mutex.
    // This prevents the race where rdev fires the stop-button click
    // before the session lock is acquired.
    state.capture_stop_flag.store(true, Ordering::SeqCst);

    {
        let mut status = state
            .recording_status
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if *status != RecordingStatus::Recording {
            return Err("Keine aktive Aufnahme.".into());
        }
        *status = RecordingStatus::Processing;
        crate::error_reports::set_phase(crate::error_reports::Phase::Processing);
    }

    // Extract everything from the session in one scoped block
    let (in_flight_counter, failed_counter, audio, output_dir_path, guide_title) = {
        let mut session_guard = state
            .current_session
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let session = session_guard.as_mut().ok_or("No active session")?;

        // Clear session's copy of the flag (already set above)
        session.stop_flag.take();

        // Stop input hooks
        if let Some(hook) = session.input_hook.take() {
            hook.stop();
        }

        // Stops the warm-ups. Steps still in their check finish on their
        // own: each holds the checker, not the watch.
        session.password_watch.take();
        // Ends a visit to an unrecorded window still open and writes it.
        session.unrecorded_watch.take();

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
        audio
            .stop()
            .map_err(|e| format!("Audio stop failed: {}", e))?;
    }

    // Wait for any in-flight screenshot captures to finish writing to disk
    // BEFORE returning, so the review screen opens onto a stable filesystem
    // state. On a fast machine this is near-instant; on a slow one the last
    // click's capture + resize can take a second or two. Capped, so a stuck
    // capture cannot hang the stop.
    if let Some(counter) = in_flight_counter.as_ref() {
        let running = counter.load(Ordering::SeqCst);
        if running > 0 {
            log::info!("stop_recording: waiting for {running} in-flight capture(s) to finish");
        }
        let stuck = wait_for_captures(counter, CAPTURE_DRAIN_TIMEOUT).await;
        if stuck > 0 {
            log::warn!(
                "stop_recording: {stuck} in-flight capture(s) did not finish within {}s",
                CAPTURE_DRAIN_TIMEOUT.as_secs()
            );
        }
    }

    // Store in-flight counter in app state (kept as defense in depth for the
    // generate path; should always be 0 here after the wait above).
    *state
        .in_flight_captures
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = in_flight_counter;

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
            failed_captures,
            output_dir
        );
    } else {
        log::info!("Recording stopped. Output: {}", output_dir);
    }

    Ok(StoppedRecording {
        output_dir,
        failed_captures,
    })
}
