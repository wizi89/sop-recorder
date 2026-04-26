use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::commands::auth::SessionCache;
use crate::network::{auth as net_auth, jobs, sse, upload};
use crate::output::{markdown, pdf, pending, step_meta};
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

    // Refresh access token before upload to prevent expiration errors.
    // Supabase JWTs expire after ~1 hour; refreshing here ensures every
    // generation uses a valid token, even after long recording sessions.
    let api_base = super::auth::get_api_base(&app);
    let access_token = match net_auth::refresh_session(api_base).await {
        Ok(Some(auth)) => {
            let token = auth.access_token.clone();
            *session.access_token.lock().unwrap() = Some(auth.access_token);
            if let Some(email) = auth.email {
                *session.email.lock().unwrap() = Some(email);
            }
            log::info!("Token refreshed before upload");
            token
        }
        Ok(None) | Err(_) => {
            // Refresh failed -- fall back to cached token
            log::warn!("Token refresh failed, using cached token");
            session
                .access_token
                .lock()
                .unwrap()
                .clone()
                .ok_or("Not logged in")?
        }
    };

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

    // Read per-step sidecar JSONs alongside the PNGs. If the sidecar count
    // diverges from the screenshot count (e.g. user deleted a PNG manually
    // between stop and generate), drop the array rather than send a
    // misaligned one -- the server falls back to no per-step alignment.
    let step_metas = step_meta::read_all(&screenshots_dir);
    let steps_arg: Option<&[step_meta::StepMeta]> = if step_metas.len() == screenshot_paths.len() {
        Some(&step_metas)
    } else {
        if !step_metas.is_empty() {
            log::warn!(
                "step sidecar count ({}) does not match screenshots ({}); falling back to no alignment",
                step_metas.len(),
                screenshot_paths.len()
            );
        }
        None
    };

    // Build path refs for upload
    let path_refs: Vec<(u32, &Path)> = screenshot_paths
        .iter()
        .map(|(n, p)| (*n, p.as_path()))
        .collect();

    // Read skip_pii_check setting
    let skip_pii_check = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("skip_pii_check").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    // Read pipeline_version setting (1 = fast/v1, 2 = high quality/v2)
    let pipeline_version: u8 = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("pipeline_version").and_then(|v| v.as_u64()))
        .map(|v| v as u8)
        .unwrap_or(1);

    // Read generation_model setting
    let generation_model: String = app
        .store("settings.json")
        .ok()
        .and_then(|store| store.get("generation_model").and_then(|v| v.as_str().map(String::from)))
        .unwrap_or_else(|| "azure/gpt-4.1".to_string());

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

    // Upload with retry.
    // If the first attempt fails with 401 (token invalid despite refresh),
    // refresh the token once more and retry.  This covers clock-skew,
    // server-side revocation, and network-blip-during-refresh scenarios.
    let response = match upload::upload_with_retry(
        &access_token,
        openai_key.as_deref(),
        &audio_path,
        &path_refs,
        &guide_title,
        api_url.as_deref(),
        3,
        skip_pii_check,
        pipeline_version,
        &generation_model,
        steps_arg,
    )
    .await
    {
        Ok(resp) => resp,
        Err(e) if e.contains("(401)") => {
            log::warn!("Upload returned 401 -- attempting second token refresh");
            let fresh_token = match net_auth::refresh_session(api_base).await {
                Ok(Some(auth)) => {
                    let token = auth.access_token.clone();
                    *session.access_token.lock().unwrap() = Some(auth.access_token);
                    if let Some(email) = auth.email {
                        *session.email.lock().unwrap() = Some(email);
                    }
                    token
                }
                _ => {
                    // Refresh permanently failed -- signal frontend to re-login
                    *session.access_token.lock().unwrap() = None;
                    let _ = app.emit("auth:session_expired", ());
                    return Err("Sitzung abgelaufen. Bitte erneut anmelden.".into());
                }
            };

            upload::upload_with_retry(
                &fresh_token,
                openai_key.as_deref(),
                &audio_path,
                &path_refs,
                &guide_title,
                api_url.as_deref(),
                1, // single retry -- if this also fails, give up
                skip_pii_check,
                pipeline_version,
                &generation_model,
                steps_arg,
            )
            .await
            .map_err(|e| {
                if e.contains("(401)") {
                    // Even the fresh token was rejected -- force re-login
                    *session.access_token.lock().unwrap() = None;
                    let _ = app.emit("auth:session_expired", ());
                    "Sitzung abgelaufen. Bitte erneut anmelden.".to_string()
                } else {
                    e
                }
            })?
        }
        Err(e) => return Err(e),
    };

    // Consume SSE stream.
    // If the stream drops mid-generation (network issue), the server keeps
    // running the generation task.  We capture the job_id early so we can
    // poll for the result instead of losing it.
    let mut captured_job_id: Option<String> = None;
    let result = match sse::consume_sse_stream(response, &app, &mut captured_job_id).await {
        Ok(r) => r,
        Err(e) => {
            if let Some(ref job_id) = captured_job_id {
                log::warn!(
                    "SSE stream failed ({}) but have job_id={}, polling for result",
                    e, job_id
                );
                let _ = app.emit(
                    "sse:status",
                    sse::SSEStatusPayload {
                        message: "Verbindung unterbrochen -- warte auf Ergebnis...".into(),
                    },
                );
                jobs::poll_job_result(&access_token, job_id, api_url.as_deref(), api_base, 40).await?
            } else {
                return Err(e);
            }
        }
    };

    // Save markdown
    markdown::save_markdown(&output_path, &result.markdown)
        .map_err(|e| format!("Failed to save markdown: {}", e))?;

    // Download server-generated PDF, fall back to local generation
    let _ = app.emit(
        "sse:status",
        sse::SSEStatusPayload {
            message: "PDF wird heruntergeladen...".into(),
        },
    );

    let pdf_saved = if let Some(ref pdf_url) = result.pdf_url {
        match download_pdf(pdf_url, &output_path).await {
            Ok(()) => {
                log::info!("Server PDF downloaded to {}", output_path.join("guide.pdf").display());
                true
            }
            Err(e) => {
                log::warn!("Server PDF download failed ({}), falling back to local generation", e);
                false
            }
        }
    } else {
        log::info!("No pdf_url in result, falling back to local generation");
        false
    };

    if !pdf_saved {
        let _ = app.emit(
            "sse:status",
            sse::SSEStatusPayload {
                message: "PDF wird lokal erstellt...".into(),
            },
        );
        pdf::generate_pdf(&output_path, &guide_title, &result.enriched)
            .map_err(|e| format!("PDF generation failed: {}", e))?;
    }

    // Clear pending marker
    pending::clear_pending(&output_path);

    *state.recording_status.lock().unwrap() = RecordingStatus::Done;

    Ok(())
}

/// Download a PDF from a signed URL and save it to the output directory.
async fn download_pdf(url: &str, output_dir: &Path) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("PDF download request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("PDF download returned status {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read PDF response body: {}", e))?;

    let pdf_path = output_dir.join("guide.pdf");
    std::fs::write(&pdf_path, &bytes)
        .map_err(|e| format!("Failed to write PDF file: {}", e))?;

    Ok(())
}
