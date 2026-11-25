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
use crate::capture::password_check::{self, Checker};
use crate::capture::permits::{self, CapturePermits};
use crate::capture::screenshot;
use crate::capture::window_area::{self, Rect};
use crate::commands::settings::{read_capture_settings, CaptureSettings};
use crate::output::pending;
use crate::output::step_meta::{self, StepMeta};
use crate::state::{ActiveRecording, AppState, RecordingStatus};

mod files;
mod step;
mod stop;

pub use files::*;
pub use step::*;
pub use stop::*;

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
    let mut status = state
        .recording_status
        .lock()
        .unwrap_or_else(|e| e.into_inner());
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

    let store = app.store("settings.json").ok();
    // Apply hide-from-screenshots setting
    let hide = store
        .as_ref()
        .and_then(|store| store.get("hide_from_screenshots"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    // Read once, here, and moved into the capture callback by value: a change
    // made in the settings while this recording runs applies to the next one.
    let capture_settings = recording_capture_settings(read_capture_settings(|key| {
        store.as_ref().and_then(|store| store.get(key))
    }));
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
    let audio_handle = match AudioHandle::start(&audio_path) {
        Ok(handle) => Some(handle),
        Err(e) => {
            // No microphone, or one that will not open: the steps are still
            // worth recording. The server requires an audio file and builds
            // the guide from the screenshots alone when it is silent, so a
            // silent one keeps everything downstream unchanged; the level
            // poller below raises the usual no-sound warning after a few
            // seconds, since the level stays at exactly zero.
            log::warn!("Audio start failed, recording without narration: {e}");
            crate::capture::audio::write_silent_wav(&audio_path)?;
            None
        }
    };

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
    let audio_level_atomic = audio_handle
        .as_ref()
        .map(|h| h.audio_level.clone())
        .unwrap_or_default();
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
    // Buffer slots bound how many screenshots are resident at once; read
    // permits serialise the monitor reads (privacy-capture D3). Per recording
    // rather than global: nothing captures between recordings.
    let slots = permits::buffer_slots(screenshot::largest_monitor_pixels());
    log::info!("Recording holds at most {slots} screenshot(s) in memory at once");
    let capture_permits = Arc::new(CapturePermits::new(slots));
    let read_permits = Arc::new(CapturePermits::new(permits::PIXEL_READ_PERMITS));

    // The password check and its warm-up triggers, from the start of the
    // recording so windows already open are checked before the first step.
    #[cfg(windows)]
    let (checker, password_watch) = if capture_settings.mask_passwords {
        let budget = if capture_settings.monitor_mode {
            password_check::MONITOR_BUDGET
        } else {
            password_check::ACTIVE_WINDOW_BUDGET
        };
        let timeout_ms = budget.as_millis() as u32;
        let checker = Checker::new(
            Arc::new(crate::capture::uia::UiaQuery { timeout_ms }),
            budget,
        );
        let watch = crate::capture::uia::Watch::start(checker.clone(), timeout_ms);
        (Some(checker), Some(watch))
    } else {
        (None, None)
    };
    #[cfg(not(windows))]
    let (checker, password_watch): (Option<Arc<Checker>>, Option<password_check::Watch>) =
        (None, None);

    // Unrecorded windows (#65): outside the masking choice, since their steps
    // are lost either way. The bar hears when one comes to the front.
    #[cfg(windows)]
    let unrecorded_watch = {
        let app = app.clone();
        Some(crate::capture::unrecorded::Watch::start(
            &output_dir,
            recording_start,
            move |in_front| {
                let _ = app.emit("recording:unrecorded_window", in_front);
            },
        ))
    };
    #[cfg(not(windows))]
    let unrecorded_watch: Option<crate::capture::unrecorded::Watch> = None;

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
    let readers_clone = read_permits.clone();
    let checker_clone = checker.clone();
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
        let readers = readers_clone.clone();
        let checker = checker_clone.clone();
        // Incremented before the spawn, so the counter covers a capture that is
        // still queued for a permit as well as one already running. That is what
        // `stop_recording` and the generate path wait on, and a queued capture
        // is exactly the kind they must not race past.
        flight.fetch_add(1, Ordering::SeqCst);
        #[cfg(windows)]
        let latest = counter_clone.clone();
        std::thread::spawn(move || {
            // Phase A takes the pixels at once; phase B checks and writes. The
            // buffer slot is released on the way out however the capture ends.
            let saved = permits::two_phase(
                &permits,
                &readers,
                || screenshot::capture_step(step_num, click_pos, capture_settings.monitor_mode),
                |held| {
                    // The bar follows the click only once the pixels are read,
                    // so moving it (a call into the UI thread) never delays the
                    // read, and only while it is hidden from captures: shown,
                    // it would appear in the next screenshot of a monitor it
                    // never sat on. A save can finish seconds later, out of
                    // order with newer clicks, so only the newest click moves
                    // it, or the bar jumps back and forth.
                    #[cfg(windows)]
                    if let Some(click) =
                        click_pos.filter(|_| hide && latest.load(Ordering::SeqCst) == step_num)
                    {
                        crate::commands::window::follow_click(&app, click);
                    }
                    let held = held?;
                    let capture_scope = held.scope;
                    // Masks are filled before the PNG is encoded: the only
                    // screenshot file ever written is the masked one.
                    let (masks, password_check, window_list_partial) =
                        check_step(checker.as_ref(), &held, step_num);
                    screenshot::finish_and_save(held, &dir, step_num, click_pos, &masks).map(
                        |marker_box| SavedStep {
                            marker_box,
                            capture_scope,
                            password_check,
                            window_list_partial,
                        },
                    )
                },
            );
            let outcome = note_capture_outcome(
                saved,
                step_num,
                timestamp_seconds,
                click_pos,
                trigger,
                &failed,
            );
            match outcome {
                CaptureOutcome::Captured(meta) => {
                    // Written AFTER the PNG so a crash between the two leaves an
                    // unpaired PNG the upload path can detect, rather than a
                    // JSON pointing at a missing image.
                    if let Err(e) = step_meta::write_sidecar(&dir, &meta) {
                        log::error!("Step sidecar write failed for step {}: {}", step_num, e);
                    }
                    let _ = app.emit("recording:step_captured", step_num);
                }
                CaptureOutcome::Failed { total } => {
                    let _ = app.emit("recording:step_failed", total);
                }
            }
            flight.fetch_sub(1, Ordering::SeqCst);
        });
    });

    // Create session
    let session = ActiveRecording {
        output_dir: output_dir.clone(),
        screenshots_dir,
        guide_title,
        audio_handle,
        input_hook: Some(hook_handle),
        stop_flag: Some(stop_flag),
        in_flight: Some(in_flight),
        step_counter: Some(step_counter),
        failed_captures: Some(failed_captures),
        password_watch,
        unrecorded_watch,
    };

    *state
        .current_session
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(session);
    *status = RecordingStatus::Recording;
    // What a panic report says the app was doing (design D3). Set here rather
    // than read out of this mutex at panic time: the panic hook must not take
    // a lock it may be racing or that may already be poisoned.
    crate::error_reports::set_phase(crate::error_reports::Phase::Recording);

    let _ = app.emit("recording:started", ());
    log::info!(
        "Recording started: {} (capture: {}, password masking: {})",
        output_dir.display(),
        if capture_settings.monitor_mode {
            "monitor"
        } else {
            "active_window"
        },
        if capture_settings.mask_passwords {
            "on"
        } else {
            "off"
        },
    );
    Ok(())
}

/// Longest wait for screenshot captures still being written. On a fast machine
/// they finish at once and on a slow one within a second or two; one that
/// takes longer is stuck, and waiting on it would hang Stop or Generate.
pub const CAPTURE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Waits until no capture is in flight or `timeout` has passed, and returns how
/// many are still running: 0 when all finished.
pub async fn wait_for_captures(counter: &AtomicU32, timeout: std::time::Duration) -> u32 {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let running = counter.load(Ordering::SeqCst);
        if running == 0 || tokio::time::Instant::now() >= deadline {
            return running;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests;
