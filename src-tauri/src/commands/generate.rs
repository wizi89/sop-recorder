use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};
use tauri_plugin_store::StoreExt;

use crate::commands::auth::SessionCache;
use crate::network::{auth as net_auth, jobs, sse, upload};
use crate::output::{markdown, pending, step_meta};
use crate::state::{AppState, RecordingStatus};

static GENERATING: AtomicBool = AtomicBool::new(false);

/// The step number a screenshot filename encodes, or `None` if the name is not
/// one of ours.
///
/// Parsed rather than pattern-matched on a fixed width: the writer formats with
/// `{:02}`, so step 100 is `step_100.png`, and a width-two assumption would
/// drop every step past 99 on a long recording.
fn step_number_from_filename(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("step_")?.strip_suffix(".png")?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Every screenshot in a recording folder, in ascending step order.
///
/// Reads the directory rather than counting upward from 1. The count-and-stop
/// version this replaces ended enumeration at the first missing number, so a
/// single failed capture silently removed every later screenshot from the
/// upload -- 21 clicks shipped as one step in the 2026-09-03 test. A gap is a
/// capture that failed, not the end of the recording, and the steps after it
/// are still the user's work.
///
/// Sorting is on the parsed number, not the filename: lexicographically
/// `step_10.png` precedes `step_09.png`, which would reorder the guide the
/// moment a recording passes nine steps.
fn collect_step_screenshots(screenshots_dir: &Path) -> Result<Vec<(u32, PathBuf)>, String> {
    let entries = std::fs::read_dir(screenshots_dir)
        .map_err(|e| format!("Failed to read {}: {}", screenshots_dir.display(), e))?;

    let mut found: Vec<(u32, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let step = step_number_from_filename(&path.file_name()?.to_string_lossy())?;
            Some((step, path))
        })
        .collect();
    found.sort_by_key(|(step, _)| *step);

    if let Some((last, _)) = found.last() {
        let present: std::collections::HashSet<u32> = found.iter().map(|(n, _)| *n).collect();
        let missing: Vec<u32> = (1..=*last).filter(|n| !present.contains(n)).collect();
        if !missing.is_empty() {
            // The count is what tells a reader whether the recording lost work,
            // so it is stated even though the steps are recoverable.
            log::warn!(
                "generate: {} screenshot(s) present, {} missing from the sequence ({:?}) \
                 (failed capture or deleted in review); uploading what is on disk",
                found.len(),
                missing.len(),
                missing,
            );
        }
    }

    Ok(found)
}

/// Marks an upload refused because of the review screen's boxes, so the
/// frontend returns to the review screen instead of the error screen, whose
/// Retry would repeat the same refusal (review-redaction D5).
pub const REDACTION_REFUSAL: &str = "[redaction] ";

fn refuse(msg: String) -> String {
    format!("{REDACTION_REFUSAL}{msg}")
}

/// The screenshots exactly as they will be uploaded, built once before the
/// upload loop (review-redaction D2): steps with boxes are decoded, filled and
/// re-encoded in memory; every other step is its file's bytes unchanged. Every
/// attempt, including the 401 retry, sends this list, so no retry can reach
/// the originals on disk.
///
/// Refuses, naming the step, when the boxes cannot be read, a screenshot with
/// boxes cannot be decoded, or a box entry or sidecar has no screenshot (a
/// delete that failed part way: the box may belong to a step that would
/// otherwise go out unredacted).
fn build_upload_screenshots(
    screenshots_dir: &Path,
    screenshots: &[(u32, PathBuf)],
) -> Result<Vec<upload::UploadScreenshot>, String> {
    let boxes = crate::output::redactions::read(screenshots_dir).map_err(|e| {
        refuse(format!(
            "Die Schwärzungen dieser Aufnahme sind nicht lesbar, es wurde nichts hochgeladen. ({e})"
        ))
    })?;
    let present: std::collections::BTreeSet<u32> = screenshots.iter().map(|(n, _)| *n).collect();
    if let Some(step) =
        super::review::leftover_steps(screenshots_dir, &present, Some(&boxes)).first()
    {
        return Err(refuse(format!(
            "Schritt {step} wurde nicht vollständig gelöscht, es wurde nichts hochgeladen. \
             Bitte den Schritt erneut löschen."
        )));
    }

    screenshots
        .iter()
        .map(|(order, path)| {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("Failed to read screenshot {}: {}", order, e))?;
            let bytes = match boxes.get(order) {
                Some(list) if !list.is_empty() => burn(&bytes, list).map_err(|e| {
                    refuse(format!(
                        "Schritt {order} konnte nicht geschwärzt werden, es wurde nichts \
                         hochgeladen. ({e})"
                    ))
                })?,
                _ => bytes,
            };
            Ok(upload::UploadScreenshot {
                order: *order,
                path: path.clone(),
                bytes,
            })
        })
        .collect()
}

/// Decode a PNG, fill the boxes, encode it again.
fn burn(png: &[u8], boxes: &[crate::capture::screenshot::PixelBox]) -> Result<Vec<u8>, String> {
    let mut img = image::load_from_memory(png)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    crate::capture::screenshot::apply_boxes(&mut img, boxes);
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// The per-step metadata to send with an upload, or `None` to send none.
///
/// The server pairs `metadata.steps` to the screenshots by position and only
/// when the two lengths agree (`routes_generate.py`); it never reads `order`.
/// So a divergence has to drop the array outright -- shipping a shorter one
/// would describe every step after the divergence against the wrong image,
/// which is worse than having no per-step transcript at all.
fn steps_for_upload(
    metas: &[step_meta::StepMeta],
    screenshot_count: usize,
) -> Option<&[step_meta::StepMeta]> {
    if metas.len() == screenshot_count && !metas.is_empty() {
        return Some(metas);
    }
    // Logged even when there are no sidecars at all: "none were readable" and
    // "there were none" look identical in the resulting guide, and the
    // 2026-09-03 test showed what silence here costs.
    log::warn!(
        "generate: alignment dropped -- {} sidecar(s) for {} screenshot(s); \
         the guide will have no per-step transcript",
        metas.len(),
        screenshot_count,
    );
    None
}

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

    crate::error_reports::set_phase(crate::error_reports::Phase::Processing);
    let result = run_generation_inner(output_dir, app, session, state.clone()).await;
    GENERATING.store(false, Ordering::SeqCst);
    if result.is_err() {
        // Here rather than at each early return, so a new one cannot forget
        // it and leave the app showing "Processing" with nothing processing.
        *state
            .recording_status
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = RecordingStatus::Idle;
    }
    crate::error_reports::set_phase(crate::error_reports::Phase::Idle);
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
    let in_flight = state
        .in_flight_captures
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(counter) = in_flight {
        if counter.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            let _ = app.emit(
                "sse:status",
                sse::SSEStatusPayload {
                    message: "Screenshots werden verarbeitet...".into(),
                },
            );
            let stuck = super::recording::wait_for_captures(
                &counter,
                super::recording::CAPTURE_DRAIN_TIMEOUT,
            )
            .await;
            // Generating without the stuck ones beats waiting forever: the
            // steps that did finish are on disk, and the guide is built
            // from what is there.
            if stuck > 0 {
                log::warn!("run_generation: going ahead with {stuck} capture(s) still unfinished");
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
            *session
                .access_token
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(auth.access_token);
            if let Some(email) = auth.email {
                *session.email.lock().unwrap_or_else(|e| e.into_inner()) = Some(email);
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

    // Collect audio + screenshot paths
    let audio_path = output_path.join("recording.wav");
    if !audio_path.exists() {
        return Err("Audio file not found".into());
    }

    let screenshots_dir = output_path.join("screenshots");
    let screenshot_paths = collect_step_screenshots(&screenshots_dir)?;

    log::info!(
        "generate: {} screenshot(s) from {} (steps {:?}..{:?})",
        screenshot_paths.len(),
        screenshots_dir.display(),
        screenshot_paths.first().map(|(n, _)| *n),
        screenshot_paths.last().map(|(n, _)| *n),
    );

    if screenshot_paths.is_empty() {
        return Err("No screenshots found".into());
    }

    // Get guide title from pending.json or dirname
    let guide_title = pending::read_pending(&output_path)
        .and_then(|meta| {
            meta.get("guide_title")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| {
            output_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "SOP".to_string())
        });

    // Read per-step sidecar JSONs alongside the PNGs. The server pairs these to
    // the screenshots positionally and only when the two lengths agree, so a
    // divergence (a PNG deleted by hand, a sidecar that failed to write) has to
    // drop the array rather than ship a misaligned one: every step after the
    // divergence would otherwise be described against the wrong image.
    //
    // Both sides survive gaps now, so the ordinary failed-capture case keeps
    // its alignment instead of losing the recording's narration.
    let step_metas = step_meta::read_all(&screenshots_dir);
    let steps_arg = steps_for_upload(&step_metas, screenshot_paths.len());

    // The bytes to upload, boxes burned in, built once for every attempt.
    let upload_screenshots = build_upload_screenshots(&screenshots_dir, &screenshot_paths)?;

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
        .and_then(|store| {
            store
                .get("generation_model")
                .and_then(|v| v.as_str().map(String::from))
        })
        .unwrap_or_else(|| "azure/gpt-5.4".to_string());

    // The user's pipeline choice from the review screen. Independent of the
    // advanced-settings gate: every org may pick a pipeline, no org sees the
    // pipeline_version / model / upload-target controls unless allowlisted.
    let pipeline_id = super::pipelines::selected_pipeline_id(&app);

    // Honor upload_target unconditionally. The Settings UI only exposes
    // the dropdown to orgs in ADVANCED_SETTINGS_ORGS (server-driven via
    // features.advanced_settings), so end users can't accidentally route
    // their traffic away from production.
    let api_url = app
        .store("settings.json")
        .ok()
        .and_then(|store| {
            store
                .get("upload_target")
                .and_then(|v| v.as_str().map(String::from))
        })
        .map(|target| net_auth::api_url_for_target(Some(&target)).to_string());

    // Upload with retry.
    // If the first attempt fails with 401 (token invalid despite refresh),
    // refresh the token once more and retry.  This covers clock-skew,
    // server-side revocation, and network-blip-during-refresh scenarios.
    let response = match upload::upload_with_retry(
        &access_token,
        &audio_path,
        &upload_screenshots,
        &guide_title,
        api_url.as_deref(),
        3,
        skip_pii_check,
        pipeline_version,
        &generation_model,
        steps_arg,
        pipeline_id.as_deref(),
    )
    .await
    {
        Ok(resp) => resp,
        Err(e) if e.contains("(401)") => {
            log::warn!("Upload returned 401 -- attempting second token refresh");
            let fresh_token = match net_auth::refresh_session(api_base).await {
                Ok(Some(auth)) => {
                    let token = auth.access_token.clone();
                    *session
                        .access_token
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(auth.access_token);
                    if let Some(email) = auth.email {
                        *session.email.lock().unwrap_or_else(|e| e.into_inner()) = Some(email);
                    }
                    token
                }
                _ => {
                    // Refresh permanently failed -- signal frontend to re-login
                    *session
                        .access_token
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = None;
                    let _ = app.emit("auth:session_expired", ());
                    return Err("Sitzung abgelaufen. Bitte erneut anmelden.".into());
                }
            };

            upload::upload_with_retry(
                &fresh_token,
                &audio_path,
                &upload_screenshots,
                &guide_title,
                api_url.as_deref(),
                1, // single retry -- if this also fails, give up
                skip_pii_check,
                pipeline_version,
                &generation_model,
                steps_arg,
                pipeline_id.as_deref(),
            )
            .await
            .map_err(|e| {
                if e.contains("(401)") {
                    // Even the fresh token was rejected -- force re-login
                    *session
                        .access_token
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = None;
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
                // Not a failure: the server keeps running the job, and the
                // result is fetched by polling. Reported to the user as the
                // recovery it is -- "unterbrochen" read as data loss in the
                // 2026-09-03 test for two generations that both completed.
                log::warn!(
                    "SSE stream dropped ({}); job_id={} is still running, \
                     polling for its result",
                    e,
                    job_id
                );
                let _ = app.emit(
                    "sse:status",
                    sse::SSEStatusPayload {
                        message:
                            "Verbindung wird wiederhergestellt -- die Verarbeitung läuft weiter..."
                                .into(),
                    },
                );
                jobs::poll_job_result(&access_token, job_id, api_url.as_deref(), api_base, 40)
                    .await?
            } else {
                return Err(e);
            }
        }
    };

    // Save markdown
    markdown::save_markdown(&output_path, &result.markdown)
        .map_err(|e| format!("Failed to save markdown: {}", e))?;

    // The PDF is the server's. Without it the guide is still complete -- on
    // the server and as markdown here -- so a missing PDF does not fail the
    // generation. There used to be a local fallback, but it only found fonts
    // on Windows: on macOS it turned this case into a failed generation.
    let _ = app.emit(
        "sse:status",
        sse::SSEStatusPayload {
            message: "PDF wird heruntergeladen...".into(),
        },
    );

    match result.pdf_url.as_deref() {
        Some(pdf_url) => match download_pdf(pdf_url, &output_path).await {
            Ok(()) => log::info!(
                "Server PDF downloaded to {}",
                output_path.join("guide.pdf").display()
            ),
            Err(e) => log::warn!("Server PDF download failed ({}); no PDF saved", e),
        },
        None => log::warn!("No pdf_url in result; no PDF saved"),
    }

    // Clear pending marker
    pending::clear_pending(&output_path);

    *state
        .recording_status
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = RecordingStatus::Done;

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
        return Err(format!(
            "PDF download returned status {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read PDF response body: {}", e))?;

    let pdf_path = output_dir.join("guide.pdf");
    std::fs::write(&pdf_path, &bytes).map_err(|e| format!("Failed to write PDF file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"not really a png").unwrap();
    }

    fn steps(dir: &Path) -> Vec<u32> {
        collect_step_screenshots(dir)
            .unwrap()
            .into_iter()
            .map(|(n, _)| n)
            .collect()
    }

    /// The 2026-09-03 regression. A capture failed at step 2; the counting loop
    /// this replaces stopped there and shipped one screenshot out of three.
    #[test]
    fn a_gap_does_not_end_the_sequence() {
        let dir = tempdir().unwrap();
        for name in ["step_01.png", "step_03.png", "step_04.png"] {
            touch(dir.path(), name);
        }

        assert_eq!(steps(dir.path()), vec![1, 3, 4]);
    }

    /// Lexicographic order puts step 10 before step 9. Sorting on the parsed
    /// number is what keeps a recording longer than nine steps in order.
    #[test]
    fn steps_are_ordered_numerically_not_lexicographically() {
        let dir = tempdir().unwrap();
        for step in 1..=21u32 {
            touch(dir.path(), &format!("step_{:02}.png", step));
        }

        assert_eq!(steps(dir.path()), (1..=21).collect::<Vec<_>>());
    }

    #[test]
    fn unrelated_files_are_ignored() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "step_01.png");
        for name in [
            "step_01.json",
            "step_.png",
            "step_2a.png",
            "notes.txt",
            "step_01.png.bak",
        ] {
            touch(dir.path(), name);
        }

        assert_eq!(steps(dir.path()), vec![1]);
    }

    #[test]
    fn an_empty_folder_yields_no_steps() {
        let dir = tempdir().unwrap();
        assert!(collect_step_screenshots(dir.path()).unwrap().is_empty());
    }

    /// A folder that is not there at all is an error rather than an empty
    /// result, so the caller can name it instead of reporting "no screenshots"
    /// for a recording that was never written.
    #[test]
    fn a_missing_folder_is_an_error_naming_it() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("screenshots");

        let err = collect_step_screenshots(&missing).unwrap_err();
        assert!(err.contains("screenshots"), "{}", err);
    }

    #[test]
    fn step_numbers_past_ninety_nine_still_parse() {
        assert_eq!(step_number_from_filename("step_100.png"), Some(100));
        assert_eq!(step_number_from_filename("step_01.png"), Some(1));
        assert_eq!(step_number_from_filename("step_1.png"), Some(1));
        assert_eq!(step_number_from_filename("step_01.json"), None);
        assert_eq!(step_number_from_filename("step_.png"), None);
        assert_eq!(step_number_from_filename("recording.wav"), None);
    }

    fn png(dir: &Path, step: u32, w: u32, h: u32) -> PathBuf {
        let path = dir.join(format!("step_{:02}.png", step));
        image::RgbaImage::from_pixel(w, h, image::Rgba([200, 210, 220, 255]))
            .save(&path)
            .unwrap();
        path
    }

    fn upload_list(dir: &Path) -> Result<Vec<upload::UploadScreenshot>, String> {
        build_upload_screenshots(dir, &collect_step_screenshots(dir).unwrap())
    }

    fn decoded(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory(bytes).unwrap().to_rgba8()
    }

    const BLACK: image::Rgba<u8> = image::Rgba([0, 0, 0, 255]);

    #[test]
    fn no_boxes_uploads_the_files_byte_identical() {
        let dir = tempdir().unwrap();
        let paths = [png(dir.path(), 1, 8, 8), png(dir.path(), 2, 8, 8)];
        let list = upload_list(dir.path()).unwrap();
        for (shot, path) in list.iter().zip(&paths) {
            assert_eq!(shot.bytes, std::fs::read(path).unwrap());
        }
    }

    #[test]
    fn boxes_are_opaque_inside_and_the_rest_is_unchanged() {
        let dir = tempdir().unwrap();
        let path = png(dir.path(), 1, 10, 10);
        let before = std::fs::read(&path).unwrap();
        crate::output::redactions::save_step(dir.path(), 1, vec![[2, 2, 3, 3], [8, 8, 5, 5]])
            .unwrap();
        let list = upload_list(dir.path()).unwrap();
        let img = decoded(&list[0].bytes);
        for (x, y, p) in img.enumerate_pixels() {
            let inside = ((2..5).contains(&x) && (2..5).contains(&y)) || (x >= 8 && y >= 8);
            assert_eq!(*p == BLACK, inside, "pixel {x},{y}");
        }
        // The original on disk is untouched.
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    /// The list is built once; every attempt sends it. Even with the files
    /// gone from disk afterwards, it still holds the burned bytes.
    #[test]
    fn the_upload_list_holds_the_burned_bytes_independent_of_the_file() {
        let dir = tempdir().unwrap();
        let path = png(dir.path(), 1, 4, 4);
        crate::output::redactions::save_step(dir.path(), 1, vec![[0, 0, 4, 4]]).unwrap();
        let list = upload_list(dir.path()).unwrap();
        let first = list[0].bytes.clone();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(list[0].bytes, first);
        assert!(decoded(&first).pixels().all(|p| *p == BLACK));
    }

    #[test]
    fn corrupt_boxes_upload_nothing() {
        let dir = tempdir().unwrap();
        png(dir.path(), 1, 4, 4);
        std::fs::write(dir.path().join("redactions.json"), "{").unwrap();
        let err = upload_list(dir.path()).unwrap_err();
        assert!(err.starts_with(REDACTION_REFUSAL), "{err}");
    }

    #[test]
    fn a_box_entry_without_a_screenshot_uploads_nothing_and_names_the_step() {
        let dir = tempdir().unwrap();
        png(dir.path(), 1, 4, 4);
        crate::output::redactions::save_step(dir.path(), 7, vec![[0, 0, 1, 1]]).unwrap();
        let err = upload_list(dir.path()).unwrap_err();
        assert!(
            err.starts_with(REDACTION_REFUSAL) && err.contains("Schritt 7"),
            "{err}"
        );
    }

    #[test]
    fn a_sidecar_without_a_screenshot_uploads_nothing() {
        let dir = tempdir().unwrap();
        png(dir.path(), 1, 4, 4);
        std::fs::write(dir.path().join("step_02.json"), "{}").unwrap();
        let err = upload_list(dir.path()).unwrap_err();
        assert!(err.contains("Schritt 2"), "{err}");
    }

    #[test]
    fn an_undecodable_screenshot_with_boxes_uploads_nothing_and_names_the_step() {
        let dir = tempdir().unwrap();
        png(dir.path(), 1, 4, 4);
        touch(dir.path(), "step_02.png");
        crate::output::redactions::save_step(dir.path(), 2, vec![[0, 0, 1, 1]]).unwrap();
        let err = upload_list(dir.path()).unwrap_err();
        assert!(
            err.starts_with(REDACTION_REFUSAL) && err.contains("Schritt 2"),
            "{err}"
        );
    }

    #[test]
    fn a_box_partly_outside_is_clipped_and_the_upload_proceeds() {
        let dir = tempdir().unwrap();
        png(dir.path(), 1, 4, 4);
        crate::output::redactions::save_step(dir.path(), 1, vec![[2, 2, 50, 50]]).unwrap();
        let img = decoded(&upload_list(dir.path()).unwrap()[0].bytes);
        assert_eq!(img.pixels().filter(|p| **p == BLACK).count(), 4);
    }

    #[test]
    fn boxes_on_a_recording_with_a_gap_land_on_the_right_step() {
        let dir = tempdir().unwrap();
        for n in [1, 3, 4] {
            png(dir.path(), n, 4, 4);
        }
        crate::output::redactions::save_step(dir.path(), 3, vec![[0, 0, 4, 4]]).unwrap();
        let list = upload_list(dir.path()).unwrap();
        assert_eq!(
            list.iter().map(|s| s.order).collect::<Vec<_>>(),
            vec![1, 3, 4]
        );
        let black = |i: usize| decoded(&list[i].bytes).pixels().all(|p| *p == BLACK);
        assert!(!black(0) && black(1) && !black(2));
    }

    fn meta(order: u32) -> step_meta::StepMeta {
        step_meta::StepMeta {
            order,
            timestamp_seconds: order as f64,
            click_x: None,
            click_y: None,
            trigger: "enter_key".into(),
            marker_box: None,
            ..Default::default()
        }
    }

    #[test]
    fn metadata_travels_when_the_counts_agree() {
        let metas = vec![meta(1), meta(2)];
        assert!(steps_for_upload(&metas, 2).is_some());
    }

    #[test]
    fn metadata_is_dropped_when_the_counts_disagree() {
        let metas = vec![meta(1)];
        assert!(steps_for_upload(&metas, 20).is_none());
        assert!(steps_for_upload(&metas, 0).is_none());
    }

    #[test]
    fn no_sidecars_sends_no_metadata() {
        assert!(steps_for_upload(&[], 3).is_none());
        assert!(steps_for_upload(&[], 0).is_none());
    }

    /// The end-to-end property the server's positional pairing rests on: after
    /// a failed capture, the screenshots and the sidecars still describe the
    /// same steps, in the same order, at the same length -- so alignment
    /// survives rather than being dropped for the whole recording.
    #[test]
    fn a_failed_capture_keeps_its_alignment() {
        let dir = tempdir().unwrap();
        // Step 2's capture failed: no PNG, and so no sidecar either.
        for step in [1u32, 3, 4] {
            touch(dir.path(), &format!("step_{:02}.png", step));
            crate::output::step_meta::write_sidecar(dir.path(), &meta(step)).unwrap();
        }

        let screenshots = collect_step_screenshots(dir.path()).unwrap();
        let metas = crate::output::step_meta::read_all(dir.path());

        assert_eq!(screenshots.len(), metas.len());
        assert_eq!(
            screenshots.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            metas.iter().map(|m| m.order).collect::<Vec<_>>(),
            "screenshot and sidecar order must match position for position",
        );
        assert!(steps_for_upload(&metas, screenshots.len()).is_some());
    }
}
