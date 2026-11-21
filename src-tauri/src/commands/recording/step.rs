//! Phase B's per-step helpers: what a capture's outcome means, the password
//! check, and the settings a recording runs with.

use super::*;

/// What one capture's outcome means for the recording.
///
/// Separated from the thread that produces it so the counting can be tested:
/// the closure it came from needs an `AppHandle`, a screen and a live session,
/// and the thing worth testing is none of those.
#[derive(Debug, PartialEq)]
pub enum CaptureOutcome {
    /// The PNG is on disk. The sidecar describing it still has to be written.
    Captured(Box<StepMeta>),
    /// The capture failed. Carries the running failure count for this
    /// recording, which is what the user is eventually shown.
    Failed { total: u32 },
}

/// Record one capture's outcome and count it if it failed.
///
/// A failure used to be only a log line. The step counter had already advanced,
/// so the recording carried a gap nobody could see, and the review screen
/// counted the screenshots that survived and looked complete -- which is how a
/// 21-click recording presented itself as a one-step guide on 2026-09-03.
/// What phase B wrote for one step.
#[derive(Debug, PartialEq)]
pub struct SavedStep {
    pub marker_box: Option<screenshot::MarkerBox>,
    pub capture_scope: step_meta::CaptureScope,
    pub password_check: step_meta::PasswordCheck,
}

pub(super) fn note_capture_outcome(
    result: Result<SavedStep, String>,
    step_num: u32,
    timestamp_seconds: f64,
    click_pos: Option<(i32, i32)>,
    trigger: &str,
    failed: &AtomicU32,
) -> CaptureOutcome {
    match result {
        Ok(saved) => CaptureOutcome::Captured(Box::new(StepMeta {
            order: step_num,
            timestamp_seconds,
            click_x: click_pos.map(|(x, _)| x),
            click_y: click_pos.map(|(_, y)| y),
            trigger: trigger.to_string(),
            marker_box: saved.marker_box,
            capture_scope: saved.capture_scope,
            password_check: saved.password_check,
        })),
        Err(e) => {
            let total = failed.fetch_add(1, Ordering::SeqCst) + 1;
            log::error!(
                "Screenshot capture failed for step {} ({} failed so far): {}",
                step_num,
                total,
                e
            );
            CaptureOutcome::Failed { total }
        }
    }
}

/// Phase B's password check (privacy-capture D4-D6): what to mask, and what
/// the step's sidecar records. `None` checker means masking is off.
pub(super) fn check_step(
    checker: Option<&Arc<Checker>>,
    held: &screenshot::HeldStep,
    step_num: u32,
) -> (Vec<Rect>, step_meta::PasswordCheck) {
    let Some(checker) = checker else {
        return (Vec::new(), step_meta::PasswordCheck::Off);
    };
    let Some(geometry) = &held.geometry else {
        log::warn!("Step {step_num}: password check flagged: no window geometry");
        return (Vec::new(), step_meta::PasswordCheck::Flagged);
    };
    let windows = window_area::visible_windows(geometry, &held.shown());
    let started = Instant::now();
    // A foreground change matters only where it picks the area: the active
    // window mode, unless the area is the taskbar alone (8.1 run).
    let foreground_changed = geometry.foreground_changed
        && held.scope == step_meta::CaptureScope::Window
        && !window_area::shell_only(&geometry.snapshots[0], &held.shown());
    let mut outcome = checker.check(&windows, held.pixels_at, foreground_changed);
    if geometry.snapshots.iter().any(|s| s.partial) {
        outcome
            .flags
            .push("the window list left out shell windows such as notifications".to_string());
    }
    if outcome.flags.is_empty() {
        log::info!(
            "Step {step_num}: password check ok, {} window(s), {} field area(s) masked, {} ms",
            windows.len(),
            outcome.masks.len(),
            started.elapsed().as_millis()
        );
        (outcome.masks, step_meta::PasswordCheck::Ok)
    } else {
        log::warn!(
            "Step {step_num}: password check flagged, {} field area(s) masked, {} ms: {}",
            outcome.masks.len(),
            started.elapsed().as_millis(),
            outcome.flags.join("; ")
        );
        (outcome.masks, step_meta::PasswordCheck::Flagged)
    }
}

/// The privacy settings a recording runs with. macOS keeps monitor capture and
/// no password masking in this version, whatever is stored (privacy-capture
/// D10).
pub(super) fn recording_capture_settings(stored: CaptureSettings) -> CaptureSettings {
    if cfg!(windows) {
        stored
    } else {
        CaptureSettings {
            monitor_mode: true,
            mask_passwords: false,
        }
    }
}
