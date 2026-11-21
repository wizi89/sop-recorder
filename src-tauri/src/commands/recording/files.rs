//! A recording's folder on disk: its screenshots, capture records and
//! unfinished recordings, and deleting the last step.

use super::*;

/// Read the raw bytes of a screenshot file. Returns a byte array the
/// frontend can wrap in a Blob to display as a thumbnail without needing
/// the Tauri asset protocol to be configured.
#[tauri::command]
pub async fn read_screenshot_bytes(path: String) -> Result<Vec<u8>, String> {
    fs::read(&path).map_err(|e| format!("Failed to read {}: {}", path, e))
}

/// A recording that was stopped but never generated.
#[derive(serde::Serialize)]
pub struct UnfinishedRecording {
    pub output_dir: String,
    pub guide_title: String,
}

/// How far back an unfinished recording is still offered at startup.
pub(super) const UNFINISHED_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(7 * 24 * 3600);

/// The newest recording from the last week that was stopped but never
/// generated -- the app was closed on the review screen, Generate failed, or
/// the app crashed after Stop. Offered at startup so it is not lost.
#[tauri::command]
pub async fn unfinished_recording(app: tauri::AppHandle) -> Option<UnfinishedRecording> {
    let dir = pending::find_pending(
        &get_output_dir(&app),
        std::time::SystemTime::now(),
        UNFINISHED_MAX_AGE,
    )?;
    let guide_title = pending::read_pending(&dir)
        .and_then(|meta| meta["guide_title"].as_str().map(String::from))
        .unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into());
    Some(UnfinishedRecording {
        output_dir: dir.to_string_lossy().into(),
        guide_title,
    })
}

/// The user discarded a stopped recording: it is no longer offered at
/// startup. Its files stay on disk.
#[tauri::command]
pub async fn discard_recording(output_dir: String) {
    pending::clear_pending(&PathBuf::from(output_dir));
}

/// One screenshot of a recording: the step number from its file name, and
/// its absolute path for `read_screenshot_bytes`.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct SessionScreenshot {
    pub step: u32,
    pub path: String,
}

/// List the captured screenshot files in a session output directory, in
/// step order. The step number is parsed from the file name, so a gap left
/// by a failed capture or a deleted step keeps every other step's number
/// (review-redaction D4); the review screen labels and addresses steps by it.
///
/// This works without an active session: it just scans the given
/// `{output_dir}/screenshots/` folder, so it can be called during the
/// review screen even after `stop_recording` has finalized the session.
#[tauri::command]
pub async fn list_session_screenshots(
    output_dir: String,
) -> Result<Vec<SessionScreenshot>, String> {
    Ok(session_screenshots(&output_dir)?
        .into_iter()
        .map(|(step, p)| SessionScreenshot {
            step,
            path: p.to_string_lossy().to_string(),
        })
        .collect())
}

pub(crate) fn session_screenshots(output_dir: &str) -> Result<Vec<(u32, PathBuf)>, String> {
    let screenshots_dir = PathBuf::from(output_dir).join("screenshots");
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
    Ok(entries)
}

/// How one screenshot was captured, for the review screen's marks
/// (privacy-capture D7).
#[derive(serde::Serialize)]
pub struct StepCaptureRecord {
    pub step: u32,
    pub path: String,
    pub capture_scope: step_meta::CaptureScope,
    pub password_check: step_meta::PasswordCheck,
}

/// The capture record of every screenshot in the folder, in the order of
/// `list_session_screenshots`. A screenshot without a readable sidecar reads
/// as `unknown` on both, so a legacy recording is marked rather than trusted.
#[tauri::command]
pub async fn list_capture_records(output_dir: String) -> Result<Vec<StepCaptureRecord>, String> {
    let screenshots = session_screenshots(&output_dir)?;
    let orders: Vec<u32> = screenshots.iter().map(|(n, _)| *n).collect();
    let records =
        step_meta::capture_records(&PathBuf::from(&output_dir).join("screenshots"), &orders);
    Ok(screenshots
        .into_iter()
        .zip(records)
        .map(
            |((step, path), (capture_scope, password_check))| StepCaptureRecord {
                step,
                path: path.to_string_lossy().to_string(),
                capture_scope,
                password_check,
            },
        )
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
    let session_guard = state
        .current_session
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let session = session_guard.as_ref().ok_or("Keine aktive Aufnahme.")?;

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
