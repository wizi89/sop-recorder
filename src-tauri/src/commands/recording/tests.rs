use super::*;
use crate::capture::screenshot::MarkerBox;

#[test]
fn macos_records_the_monitor_without_masking() {
    let stored = CaptureSettings {
        monitor_mode: false,
        mask_passwords: true,
    };
    let used = recording_capture_settings(stored);
    if cfg!(windows) {
        assert_eq!(used, stored);
    } else {
        assert!(used.monitor_mode && !used.mask_passwords);
    }
}

/// The settings are read once at start (privacy-capture D8). A source
/// guard, as in `settings/tests.rs`: the property is that the capture callback
/// never reads the store, so a change made mid-recording cannot reach it.
#[test]
fn the_capture_callback_never_rereads_the_settings() {
    // Normalised first: a CRLF checkout must not hide the callback's end
    // (docs/findings/).
    let source = include_str!("../recording.rs").replace("\r\n", "\n");
    let start = source
        .find("start_listener_with_callback(")
        .expect("capture callback not found");
    let end = start + source[start..].find("\n    });\n").expect("callback end");
    let body = &source[start..end];
    assert!(
        !body.contains("store"),
        "the callback reads the store:\n{body}"
    );
}

#[tokio::test]
async fn the_capture_wait_returns_once_the_last_capture_finishes() {
    let counter = Arc::new(AtomicU32::new(2));
    let finisher = counter.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        finisher.store(0, Ordering::SeqCst);
    });
    let started = Instant::now();
    let left = wait_for_captures(&counter, std::time::Duration::from_secs(5)).await;
    assert_eq!(left, 0);
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

#[tokio::test]
async fn a_stuck_capture_ends_the_wait_at_the_deadline() {
    let counter = AtomicU32::new(1);
    let started = Instant::now();
    let left = wait_for_captures(&counter, std::time::Duration::from_millis(200)).await;
    assert_eq!(left, 1, "the stuck capture is reported, not hidden");
    let waited = started.elapsed();
    assert!(
        waited >= std::time::Duration::from_millis(200),
        "{waited:?}"
    );
    assert!(waited < std::time::Duration::from_secs(2), "{waited:?}");
}

#[tokio::test]
async fn nothing_in_flight_means_no_wait() {
    let started = Instant::now();
    assert_eq!(
        wait_for_captures(&AtomicU32::new(0), std::time::Duration::from_secs(5)).await,
        0
    );
    assert!(started.elapsed() < std::time::Duration::from_millis(50));
}

fn box_at(x0: i32) -> Option<MarkerBox> {
    Some(MarkerBox {
        x0,
        y0: 2,
        x1: 3,
        y1: 4,
    })
}

fn saved(marker_box: Option<MarkerBox>) -> SavedStep {
    SavedStep {
        marker_box,
        capture_scope: step_meta::CaptureScope::Window,
        password_check: step_meta::PasswordCheck::Ok,
        window_list_partial: false,
    }
}

/// A partial window list marks the step whatever the masking setting: it is
/// decided from the geometry alone, before the masking-off return (#57).
#[test]
fn a_partial_window_list_is_noticed_in_either_snapshot() {
    use window_area::{Geometry, Snapshot};
    let partial = Snapshot {
        partial: true,
        ..Default::default()
    };
    assert!(!window_list_partial(None));
    assert!(!window_list_partial(Some(&Geometry::default())));
    for snapshots in [
        [partial.clone(), Snapshot::default()],
        [Snapshot::default(), partial],
    ] {
        assert!(window_list_partial(Some(&Geometry {
            snapshots,
            ..Default::default()
        })));
    }
}

/// The source guard for the same decision: the partial flag is computed
/// before `check_step` hands over to the password check, whose first line
/// returns when masking is off.
#[test]
fn the_partial_flag_does_not_depend_on_masking() {
    let source = include_str!("step.rs").replace("\r\n", "\n");
    let body = &source[source.find("pub(super) fn check_step(").unwrap()..];
    let body = &body[..body.find("\n}\n").unwrap()];
    let partial = body.find("window_list_partial(held.geometry").unwrap();
    let check = body.find("password_check_step(checker").unwrap();
    assert!(partial < check, "{body}");
}

#[tokio::test]
async fn screenshots_are_listed_with_the_step_from_their_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let shots = dir.path().join("screenshots");
    std::fs::create_dir(&shots).unwrap();
    for n in [4u32, 1, 3] {
        std::fs::write(shots.join(format!("step_{n:02}.png")), b"png").unwrap();
    }
    std::fs::write(shots.join("step_01.json"), b"{}").unwrap();
    let listed = list_session_screenshots(dir.path().to_string_lossy().to_string())
        .await
        .unwrap();
    assert_eq!(
        listed.iter().map(|s| s.step).collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
    assert!(listed[1].path.ends_with("step_03.png"));
}

/// The path the 2026-09-03 recordings took silently: a capture fails, the
/// step number has already advanced, and nothing counts the loss.
#[test]
fn a_failed_capture_is_counted() {
    let failed = AtomicU32::new(0);

    let outcome = note_capture_outcome(
        Err("Failed to save screenshot: Permission denied".into()),
        3,
        1.5,
        Some((10, 20)),
        "mouse_click",
        &failed,
    );

    assert_eq!(outcome, CaptureOutcome::Failed { total: 1 });
    assert_eq!(failed.load(Ordering::SeqCst), 1);
}

/// The count is what the review screen shows, so it has to be the running
/// total for the recording, not a per-capture flag.
#[test]
fn repeated_failures_accumulate() {
    let failed = AtomicU32::new(0);
    let totals: Vec<u32> = (1..=3)
        .map(|step| {
            match note_capture_outcome(
                Err("disk full".into()),
                step,
                step as f64,
                None,
                "enter_key",
                &failed,
            ) {
                CaptureOutcome::Failed { total } => total,
                other => panic!("expected a failure, got {:?}", other),
            }
        })
        .collect();

    assert_eq!(totals, vec![1, 2, 3]);
    assert_eq!(failed.load(Ordering::SeqCst), 3);
}

#[test]
fn a_successful_capture_does_not_touch_the_failure_count() {
    let failed = AtomicU32::new(2);

    let outcome = note_capture_outcome(
        Ok(saved(box_at(1))),
        7,
        12.25,
        Some((100, 200)),
        "mouse_click",
        &failed,
    );

    match outcome {
        CaptureOutcome::Captured(meta) => {
            assert_eq!(meta.order, 7);
            assert_eq!(meta.timestamp_seconds, 12.25);
            assert_eq!(meta.click_x, Some(100));
            assert_eq!(meta.click_y, Some(200));
            assert_eq!(meta.trigger, "mouse_click");
            assert_eq!(meta.marker_box, box_at(1));
            assert_eq!(meta.capture_scope, step_meta::CaptureScope::Window);
            assert_eq!(meta.password_check, step_meta::PasswordCheck::Ok);
        }
        other => panic!("expected a capture, got {:?}", other),
    }
    assert_eq!(failed.load(Ordering::SeqCst), 2, "unchanged by a success");
}

/// A keypress carries no cursor position and draws no marker. The server
/// reads that absence together with `trigger` to tell "no marker" from
/// "marker, position unknown", so it must be absent rather than zeroed.
#[test]
fn a_keypress_step_records_no_position_and_no_marker() {
    let failed = AtomicU32::new(0);

    match note_capture_outcome(Ok(saved(None)), 1, 0.5, None, "enter_key", &failed) {
        CaptureOutcome::Captured(meta) => {
            assert_eq!(meta.click_x, None);
            assert_eq!(meta.click_y, None);
            assert_eq!(meta.marker_box, None);
            assert_eq!(meta.trigger, "enter_key");
        }
        other => panic!("expected a capture, got {:?}", other),
    }
}

/// Failures are counted from capture threads that run concurrently behind
/// the permit pool, so the counter has to survive that.
#[test]
fn concurrent_failures_are_all_counted() {
    let failed = Arc::new(AtomicU32::new(0));

    let handles: Vec<_> = (1..=16u32)
        .map(|step| {
            let failed = failed.clone();
            std::thread::spawn(move || {
                note_capture_outcome(
                    Err("capture failed".into()),
                    step,
                    step as f64,
                    None,
                    "mouse_click",
                    &failed,
                );
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(failed.load(Ordering::SeqCst), 16);
}

/// The real password check on the real desktop, end to end:
/// `SPIKE_OUT=<dir> cargo test --lib live_password_check -- --ignored --nocapture`
/// with a login page open, then look at the PNG.
#[cfg(windows)]
#[test]
#[ignore]
fn live_password_check() {
    // What main.rs does: without it UIA reports scaled rectangles while DWM
    // reports physical ones, and every mask lands in the wrong place.
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwareness(
            windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE,
        );
    }
    let dir = PathBuf::from(std::env::var("SPIKE_OUT").expect("SPIKE_OUT"));
    let budget = password_check::MONITOR_BUDGET;
    let checker = Checker::new(
        Arc::new(crate::capture::uia::UiaQuery {
            timeout_ms: budget.as_millis() as u32,
        }),
        budget,
    );
    let _watch = crate::capture::uia::Watch::start(checker.clone(), budget.as_millis() as u32);
    std::thread::sleep(std::time::Duration::from_secs(3));
    for (hwnd, completions, fields) in checker.debug_cache() {
        println!("cache {hwnd:#x}: {completions} completion(s), fields {fields:?}");
    }

    let held = screenshot::capture_step(1, None, true).unwrap();
    let geometry = held.geometry.as_ref().unwrap();
    let windows = window_area::visible_windows(geometry, &held.shown());
    for (hwnd, parts) in &windows {
        let class = geometry.snapshots[0]
            .get(*hwnd)
            .map(|w| w.class.clone())
            .unwrap_or_default();
        println!("visible {hwnd:#x} {class} {} part(s)", parts.len());
    }
    let outcome = checker.check(&windows, held.pixels_at, geometry.foreground_changed);
    println!(
        "flags {:#?}
masks {:?}",
        outcome.flags, outcome.masks
    );
    let masks = outcome.masks;
    screenshot::finish_and_save(held, &dir, 1, None, &masks).unwrap();
}
