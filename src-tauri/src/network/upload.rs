use reqwest::multipart;
use std::path::Path;
use std::time::Duration;

use crate::output::step_meta::{self, CaptureScope, PasswordCheck, StepMeta};

const UPLOAD_TIMEOUT_SECS: u64 = 300;

/// How the uploaded screenshots were protected, counted per value
/// (privacy-capture D9). Taken from the sidecars next to the PNGs, never from
/// settings, so a regenerated or legacy folder reports what its files say; a
/// screenshot without a sidecar counts as `unknown`.
///
/// `steps` are those sidecars, already read by the caller, when they pair one
/// to one with the screenshots; only when they do not is the folder read
/// again, to count the screenshots that have one.
fn capture_counts(
    screenshot_paths: &[(u32, &Path)],
    steps: Option<&[StepMeta]>,
) -> (serde_json::Value, serde_json::Value) {
    let mut scopes = serde_json::Map::new();
    for scope in [
        CaptureScope::Window,
        CaptureScope::Monitor,
        CaptureScope::MonitorFallback,
        CaptureScope::Unknown,
    ] {
        scopes.insert(scope.as_str().into(), 0.into());
    }
    let mut checks = serde_json::Map::new();
    for check in [
        PasswordCheck::Ok,
        PasswordCheck::Flagged,
        PasswordCheck::Off,
        PasswordCheck::Unknown,
    ] {
        checks.insert(check.as_str().into(), 0.into());
    }
    let records: Vec<(CaptureScope, PasswordCheck)> = match steps {
        Some(steps) if steps.len() == screenshot_paths.len() => steps
            .iter()
            .map(|m| (m.capture_scope, m.password_check))
            .collect(),
        _ => {
            // Every screenshot of an upload lives in one `screenshots` folder.
            let dir = screenshot_paths
                .first()
                .and_then(|(_, path)| path.parent())
                .unwrap_or(Path::new("."));
            let orders: Vec<u32> = screenshot_paths.iter().map(|(order, _)| *order).collect();
            step_meta::capture_records(dir, &orders)
        }
    };
    for (scope, check) in records {
        for (map, key) in [(&mut scopes, scope.as_str()), (&mut checks, check.as_str())] {
            let count = map[key].as_u64().unwrap_or(0) + 1;
            map.insert(key.into(), count.into());
        }
    }
    (scopes.into(), checks.into())
}

/// Upload audio + screenshots to the generation endpoint.
/// Returns the SSE stream URL or response.
// ponytail: ten real parameters, not an abstraction waiting to happen.
// Bundling them into a struct would move the same list one line up and touch
// every caller of the upload path for a lint; revisit if an eleventh appears.
#[allow(clippy::too_many_arguments)]
pub async fn upload_multipart(
    access_token: &str,
    audio_path: &Path,
    screenshot_paths: &[(u32, &Path)],
    guide_title: &str,
    api_url: Option<&str>,
    skip_pii_check: bool,
    pipeline_version: u8,
    generation_model: &str,
    steps: Option<&[StepMeta]>,
    pipeline_id: Option<&str>,
) -> Result<reqwest::Response, String> {
    let base_url = api_url.unwrap_or_else(|| super::auth::api_url_for_target(None));
    let url = format!("{}/generate", base_url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(UPLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut form = multipart::Form::new();

    // Audio file
    let audio_bytes =
        std::fs::read(audio_path).map_err(|e| format!("Failed to read audio file: {}", e))?;
    let audio_part = multipart::Part::bytes(audio_bytes)
        .file_name("recording.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    form = form.part("audio", audio_part);

    // Screenshots
    for (order, path) in screenshot_paths {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read screenshot {}: {}", order, e))?;
        let field_name = format!("step_{:02}", order);
        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.png", field_name));
        let part = multipart::Part::bytes(bytes)
            .file_name(file_name)
            .mime_str("image/png")
            .map_err(|e| e.to_string())?;
        form = form.part(field_name, part);
    }

    // Metadata. `steps` is only included when sidecar count matches
    // screenshot count, so the server can rely on length-equality and treat
    // absence as "no per-step metadata available -- fall back to global
    // transcript and proportional step splitting".
    let mut metadata = serde_json::json!({
        "guide_title": guide_title,
        "step_count": screenshot_paths.len(),
        "pipeline_version": pipeline_version,
        "generation_model": generation_model,
    });
    if let Some(s) = steps {
        if s.len() == screenshot_paths.len() && !s.is_empty() {
            metadata["steps"] = serde_json::json!(s);
        }
    }
    // The user's pipeline choice, by id only. Omitted entirely when nothing is
    // selected: absence is the normal case and the server falls through to its
    // default path.
    if let Some(id) = pipeline_id.filter(|s| !s.is_empty()) {
        metadata["pipeline_id"] = serde_json::json!(id);
    }
    let (capture_scopes, password_checks) = capture_counts(screenshot_paths, steps);
    metadata["capture_scopes"] = capture_scopes;
    metadata["password_checks"] = password_checks;
    form = form.text("metadata", metadata.to_string());

    if skip_pii_check {
        form = form.text("skip_pii_check", "true");
    }

    let req = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .multipart(form);

    let response = req.send().await.map_err(|e| {
        if e.is_timeout() {
            "Request timed out (300s)".to_string()
        } else if e.is_connect() {
            "Server not reachable. Check your connection.".to_string()
        } else {
            format!("Upload failed: {}", e)
        }
    })?;

    let status = response.status();
    log::info!(
        "Upload response: {} {}",
        status.as_u16(),
        status.canonical_reason().unwrap_or("")
    );

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::error!("Upload failed: {} - {}", status, body);
        // Every refusal the server authors carries {"error": code, "message":
        // German text} written for the person at the keyboard. Show that text
        // instead of the raw JSON; the body stays in the log for support.
        // The "(4xx" prefix is load-bearing: upload_with_retry matches on it to
        // decide what not to retry.
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["message"].as_str().map(str::to_string))
            .filter(|m| !m.is_empty());
        return Err(match message {
            Some(m) => format!("({}) {}", status, m),
            None => format!("Server error ({}): {}", status, body),
        });
    }

    // For SSE, we need to return the response to stream events
    // Re-send as SSE request... actually the /generate endpoint returns SSE directly
    // So we return the response for the SSE consumer
    Ok(response)
}

/// Retry logic with exponential backoff for transient errors.
// ponytail: eleven real parameters, not an abstraction waiting to happen.
// Bundling them into a struct would move the same list one line up and touch
// every caller of the upload path for a lint; revisit if a twelfth appears.
#[allow(clippy::too_many_arguments)]
pub async fn upload_with_retry(
    access_token: &str,
    audio_path: &Path,
    screenshot_paths: &[(u32, &Path)],
    guide_title: &str,
    api_url: Option<&str>,
    max_retries: u32,
    skip_pii_check: bool,
    pipeline_version: u8,
    generation_model: &str,
    steps: Option<&[StepMeta]>,
    pipeline_id: Option<&str>,
) -> Result<reqwest::Response, String> {
    let mut last_err = String::new();
    let delays = [1, 2, 4]; // seconds

    for attempt in 0..=max_retries {
        match upload_multipart(
            access_token,
            audio_path,
            screenshot_paths,
            guide_title,
            api_url,
            skip_pii_check,
            pipeline_version,
            generation_model,
            steps,
            pipeline_id,
        )
        .await
        {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                last_err = e.clone();

                // Don't retry 4xx errors (except 429 rate-limit and
                // 401 auth errors, which the caller can recover from
                // by refreshing the token).
                if e.contains("(4") && !e.contains("(429)") && !e.contains("(401)") {
                    return Err(e);
                }
                // 401: return immediately so the caller can refresh and retry
                if e.contains("(401)") {
                    return Err(e);
                }

                if attempt < max_retries {
                    let delay = delays.get(attempt as usize).copied().unwrap_or(4);
                    log::warn!(
                        "Upload attempt {} failed: {}. Retrying in {}s...",
                        attempt + 1,
                        e,
                        delay
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }
    }

    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::step_meta::write_sidecar;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn recording(
        dir: &Path,
        steps: &[(u32, Option<(CaptureScope, PasswordCheck)>)],
    ) -> Vec<(u32, PathBuf)> {
        steps
            .iter()
            .map(|(order, record)| {
                let png = dir.join(format!("step_{:02}.png", order));
                std::fs::write(&png, b"png").unwrap();
                if let Some((capture_scope, password_check)) = record {
                    write_sidecar(
                        dir,
                        &StepMeta {
                            order: *order,
                            capture_scope: *capture_scope,
                            password_check: *password_check,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                }
                (*order, png)
            })
            .collect()
    }

    fn counts(paths: &[(u32, PathBuf)]) -> (serde_json::Value, serde_json::Value) {
        let refs: Vec<(u32, &Path)> = paths.iter().map(|(n, p)| (*n, p.as_path())).collect();
        capture_counts(&refs, None)
    }

    /// The spec's "default settings" scenario: five window steps, one flagged.
    #[test]
    fn a_normal_recording_is_counted_from_its_sidecars() {
        let dir = tempdir().unwrap();
        let mut steps: Vec<_> = (1..=4)
            .map(|n| (n, Some((CaptureScope::Window, PasswordCheck::Ok))))
            .collect();
        steps.push((5, Some((CaptureScope::Window, PasswordCheck::Flagged))));
        let (scopes, checks) = counts(&recording(dir.path(), &steps));
        assert_eq!(scopes["window"], 5);
        assert_eq!(scopes["unknown"], 0);
        assert_eq!(checks["ok"], 4);
        assert_eq!(checks["flagged"], 1);
    }

    #[test]
    fn regenerating_a_folder_reports_the_same_counts() {
        let dir = tempdir().unwrap();
        let paths = recording(
            dir.path(),
            &[
                (1, Some((CaptureScope::Monitor, PasswordCheck::Off))),
                (2, Some((CaptureScope::MonitorFallback, PasswordCheck::Ok))),
            ],
        );
        assert_eq!(counts(&paths), counts(&paths));
        assert_eq!(counts(&paths).0["monitor_fallback"], 1);
    }

    /// The upload counts from the sidecars it already read for
    /// `metadata.steps`, and gets what reading the folder again would give.
    #[test]
    fn counting_from_the_steps_already_read_matches_the_folder() {
        let dir = tempdir().unwrap();
        let paths = recording(
            dir.path(),
            &[
                (1, Some((CaptureScope::Window, PasswordCheck::Ok))),
                (
                    3,
                    Some((CaptureScope::MonitorFallback, PasswordCheck::Flagged)),
                ),
            ],
        );
        let refs: Vec<(u32, &Path)> = paths.iter().map(|(n, p)| (*n, p.as_path())).collect();
        let steps = step_meta::read_all(dir.path());
        assert_eq!(capture_counts(&refs, Some(&steps)), counts(&paths));
    }

    #[test]
    fn a_legacy_folder_reports_unknown() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("step_01.json"),
            r#"{"order":1,"timestamp_seconds":1.0,"click_x":null,"click_y":null,"trigger":"enter_key"}"#,
        )
        .unwrap();
        let paths = recording(dir.path(), &[(1, None), (2, None)]);
        let (scopes, checks) = counts(&paths);
        assert_eq!(scopes["unknown"], 2);
        assert_eq!(checks["unknown"], 2);
    }

    #[test]
    fn a_screenshot_without_a_sidecar_counts_as_unknown() {
        let dir = tempdir().unwrap();
        let paths = recording(
            dir.path(),
            &[
                (1, Some((CaptureScope::Window, PasswordCheck::Ok))),
                (2, None),
            ],
        );
        let (scopes, checks) = counts(&paths);
        assert_eq!(
            (scopes["window"].clone(), scopes["unknown"].clone()),
            (1.into(), 1.into())
        );
        assert_eq!(
            (checks["ok"].clone(), checks["unknown"].clone()),
            (1.into(), 1.into())
        );
    }
}
