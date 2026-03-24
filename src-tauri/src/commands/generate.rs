use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::commands::auth::SessionCache;
use crate::network::{auth as net_auth, sse, upload};
use crate::output::{markdown, pdf, pending};
use crate::state::{AppState, RecordingStatus};

static GENERATING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub async fn run_generation(
    output_dir: String,
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Prevent concurrent/duplicate generation calls
    if GENERATING.swap(true, Ordering::SeqCst) {
        return Err("Generation already in progress".into());
    }

    let result = run_generation_inner(output_dir, app, session, state).await;
    GENERATING.store(false, Ordering::SeqCst);
    result
}

async fn run_generation_inner(
    output_dir: String,
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let output_path = PathBuf::from(&output_dir);

    // Wait for any in-flight screenshot captures to finish
    let in_flight = state.in_flight_captures.lock().unwrap().take();
    if let Some(counter) = in_flight {
        if counter.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            let _ = app.emit("sse:status", sse::SSEStatusPayload {
                message: "Screenshots werden verarbeitet...".into(),
            });
            while counter.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }

    // Get access token
    let access_token = session
        .access_token
        .lock()
        .unwrap()
        .clone()
        .ok_or("Not logged in")?;

    // Get OpenAI key from keyring
    let openai_key = net_auth::keyring_load("openai-key")
        .ok()
        .flatten();

    // Collect audio + screenshot paths
    let audio_path = output_path.join("recording.wav");
    if !audio_path.exists() {
        return Err("Audio file not found".into());
    }

    let screenshots_dir = output_path.join("screenshots");
    let mut screenshot_paths: Vec<(u32, PathBuf)> = Vec::new();
    let mut step_num = 1u32;
    loop {
        let filename = format!("step_{:02}.png", step_num);
        let path = screenshots_dir.join(&filename);
        if path.exists() {
            screenshot_paths.push((step_num, path));
            step_num += 1;
        } else {
            break;
        }
    }

    if screenshot_paths.is_empty() {
        *state.recording_status.lock().unwrap() = RecordingStatus::Idle;
        return Err("No screenshots found".into());
    }

    // Get guide title from pending.json or dirname
    let guide_title = pending::read_pending(&output_path)
        .and_then(|meta| meta.get("guide_title").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| {
            output_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "SOP".to_string())
        });

    // Build path refs for upload
    let path_refs: Vec<(u32, &Path)> = screenshot_paths
        .iter()
        .map(|(n, p)| (*n, p.as_path()))
        .collect();

    // In dev mode, allow choosing local server via settings; in release always use production
    let api_url = if cfg!(debug_assertions) {
        app.store("settings.json")
            .ok()
            .and_then(|store| {
                store
                    .get("upload_target")
                    .and_then(|v| v.as_str().map(String::from))
            })
            .filter(|s| s == "Local")
            .map(|_| crate::config::API_URL_DEV.to_string())
    } else {
        None
    };

    // Upload with retry
    let response = upload::upload_with_retry(
        &access_token,
        openai_key.as_deref(),
        &audio_path,
        &path_refs,
        &guide_title,
        api_url.as_deref(),
        3,
    )
    .await?;

    // Consume SSE stream
    let result = sse::consume_sse_stream(response, &app).await?;

    // Save markdown
    markdown::save_markdown(&output_path, &result.markdown)
        .map_err(|e| format!("Failed to save markdown: {}", e))?;

    // Generate PDF
    let _ = app.emit(
        "sse:status",
        sse::SSEStatusPayload {
            message: "PDF wird erstellt...".into(),
        },
    );

    pdf::generate_pdf(&output_path, &guide_title, &result.enriched)
        .map_err(|e| format!("PDF generation failed: {}", e))?;

    // Clear pending marker
    pending::clear_pending(&output_path);

    *state.recording_status.lock().unwrap() = RecordingStatus::Done;

    Ok(())
}
