use super::*;
use tempfile::tempdir;

/// Every path that puts a report on disk goes through `Queue::write`, which is
/// what makes the dialog appear no matter which window raised the failure.
///
/// Asserted rather than trusted: the announcement used to sit in the panic
/// hook alone, so a report created from the settings window was written and
/// never shown. A second `std::fs::write` added elsewhere would reintroduce
/// exactly that, and it would look fine in review.
mod single_writer {
    /// Every shipping file of the module. The tests live in files of their
    /// own, so these hold no fixture that writes a file on purpose.
    const PRODUCTION: [(&str, &str); 6] = [
        ("error_reports.rs", include_str!("../error_reports.rs")),
        ("collect.rs", include_str!("collect.rs")),
        ("panic_hook.rs", include_str!("panic_hook.rs")),
        ("queue.rs", include_str!("queue.rs")),
        ("scrub.rs", include_str!("scrub.rs")),
        ("state.rs", include_str!("state.rs")),
    ];

    #[test]
    fn only_queue_write_writes_a_report_file() {
        // Assembled at runtime so the needle is not itself a match.
        let needle = format!("std::fs::{}(", "write");
        let writes: Vec<(&str, usize)> = PRODUCTION
            .iter()
            .map(|(name, source)| (*name, source.matches(&needle).count()))
            .filter(|(_, count)| *count > 0)
            .collect();
        assert_eq!(
            writes,
            vec![("queue.rs", 1)],
            "a report file is written somewhere other than `persist`; every \
             creation must go through `Queue::write`, which announces it, or \
             no dialog will open"
        );
    }
}

/// The dev triggers have to differ in the way that matters.
///
/// A command body does not run on the main thread -- async commands run on the
/// async runtime -- so `panic!()` written directly in one is a background
/// panic and the app survives it. Both panic buttons therefore did the same
/// thing, and the difference only shows up by clicking and noticing the app is
/// still alive. Asserting the shape is the only way to catch that without a
/// window on screen.
mod debug_triggers {
    const SOURCE: &str = include_str!("../commands/error_reports.rs");

    // Normalised: git hands Windows checkouts CRLF and `include_str!` embeds
    // the file as it sits on disk. The search below survives CRLF only because
    // "\r\n" happens to contain "\n", which is luck rather than intent.
    fn arm(kind: &str) -> String {
        let source = SOURCE.replace("\r\n", "\n");
        let start = source
            .find(&format!("\"{}\" =>", kind))
            .unwrap_or_else(|| panic!("no `{}` arm in debug_trigger_failure", kind));
        let rest = &source[start..];
        let end = rest.find("\n            \"").unwrap_or(rest.len());
        rest[..end].to_string()
    }

    #[test]
    fn the_main_thread_trigger_reaches_the_event_loop() {
        assert!(
            arm("main_thread_panic").contains("run_on_main_thread"),
            "a bare panic! in a command body runs on a worker thread and the \
             app survives it; the main-thread trigger must go through the app \
             handle or it is a duplicate of the background one"
        );
    }

    #[test]
    fn the_background_trigger_spawns_its_own_thread() {
        assert!(
            arm("background_panic").contains("thread::spawn"),
            "the background trigger must panic off the main thread"
        );
    }

    #[test]
    fn the_two_panic_triggers_are_not_the_same_thing() {
        assert_ne!(
            arm("main_thread_panic").contains("run_on_main_thread"),
            arm("background_panic").contains("run_on_main_thread"),
        );
    }
}

/// The diagnostic must not write outside the directory tests redirect.
mod breadcrumb_isolation {
    use super::*;
    use crate::error_reports::panic_hook::breadcrumb;

    #[test]
    fn the_trail_starts_over_instead_of_growing_without_limit() {
        let _guard = global_state_guard();
        let dir = tempdir().unwrap();
        set_active_reports_dir(dir.path().to_path_buf());
        let path = dir.path().join(TRAIL_FILE);
        std::fs::write(&path, vec![b'x'; 70 * 1024]).unwrap();

        breadcrumb("after the cap");

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("after the cap"));
        assert!(
            text.len() < 1024,
            "an oversized trail must be replaced, not appended to"
        );
    }

    #[test]
    fn the_trail_follows_the_active_reports_directory() {
        // Regression: this wrote to `dirs_next::data_local_dir()` regardless,
        // so running the suite appended to the real user's trail and produced
        // lines indistinguishable from a misbehaving app.
        let _guard = global_state_guard();
        let dir = tempdir().unwrap();
        set_active_reports_dir(dir.path().to_path_buf());

        breadcrumb("test marker");

        let trail = dir.path().join("panic-trail.log");
        assert!(
            trail.exists(),
            "the trail must land in the active directory"
        );
        assert!(std::fs::read_to_string(&trail)
            .unwrap()
            .contains("test marker"));

        let real = dirs_next::data_local_dir()
            .map(|p| p.join("com.cogniclone.recorder").join("panic-trail.log"));
        if let Some(real) = real {
            if let Ok(text) = std::fs::read_to_string(&real) {
                assert!(
                    !text.contains("test marker"),
                    "the suite wrote into the real user directory at {}",
                    real.display()
                );
            }
        }
    }
}

fn queue(dir: &tempfile::TempDir) -> Queue {
    Queue::at(dir.path().to_path_buf())
}

fn sample() -> ErrorReport {
    let mut report = ErrorReport::new(
        ReportKind::Panic,
        Phase::Recording,
        "called `Option::unwrap()` on a `None` value".to_string(),
        Some("src/capture/screenshot.rs:214".to_string()),
        vec!["[2026-09-03][12:00:01][INFO] Aufnahme gestartet".to_string()],
    );
    report.settings = Some(ReportSettings {
        upload_target: Some("Staging".to_string()),
        pipeline_version: 2,
        generation_model: "azure/gpt-5.4".to_string(),
        hide_from_screenshots: true,
        skip_pii_check: false,
    });
    report.job_id = Some("job-42".to_string());
    report.comment = Some("Ich habe auf Stopp gedrückt".to_string());
    report
}

#[test]
fn report_round_trips_through_disk() {
    let dir = tempdir().unwrap();
    let report = sample();
    let path = queue(&dir).write(&report).unwrap();
    assert_eq!(path, queue(&dir).path_of(&report.report_id));

    let read_back = queue(&dir).get(&report.report_id).unwrap();
    assert_eq!(read_back, report);
    assert_eq!(read_back.schema_version, REPORT_SCHEMA_VERSION);
}

#[test]
fn report_number_is_eight_hex_characters_of_the_id() {
    let report = sample();
    let number = report.number();
    assert_eq!(number.len(), 8);
    assert!(number.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(report.report_id.replace('-', "").starts_with(&number));
}

#[test]
fn listing_skips_an_unreadable_file_and_orders_by_time() {
    let dir = tempdir().unwrap();
    let mut older = sample();
    older.occurred_at = "2026-09-01T10:00:00+00:00".to_string();
    let mut newer = sample();
    newer.report_id = uuid::Uuid::new_v4().to_string();
    newer.occurred_at = "2026-09-02T10:00:00+00:00".to_string();
    queue(&dir).write(&older).unwrap();
    queue(&dir).write(&newer).unwrap();
    std::fs::write(dir.path().join("broken.json"), "{ not json").unwrap();

    let listed = queue(&dir).list();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].report_id, older.report_id);
    assert_eq!(listed[1].report_id, newer.report_id);
}

#[tokio::test]
async fn only_granted_reports_are_handed_to_the_sender() {
    let dir = tempdir().unwrap();
    let pending = sample();
    let mut granted = sample();
    granted.report_id = uuid::Uuid::new_v4().to_string();
    granted.consent = Consent::Granted;
    queue(&dir).write(&pending).unwrap();
    queue(&dir).write(&granted).unwrap();

    let mut handed = Vec::new();
    let sent = queue(&dir)
        .send_granted(|r| {
            handed.push(r.report_id.clone());
            async { Ok::<_, String>("a1b2c3d4".to_string()) }
        })
        .await;

    assert_eq!(handed, vec![granted.report_id.clone()]);
    assert_eq!(
        sent,
        vec![Sent {
            report_id: granted.report_id,
            number: "a1b2c3d4".to_string()
        }]
    );
    // The sent one is gone; the pending one waits for its decision.
    let left = queue(&dir).list();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].report_id, pending.report_id);
}

#[tokio::test]
async fn a_refused_send_keeps_the_report_for_the_next_try() {
    let dir = tempdir().unwrap();
    let mut report = sample();
    report.consent = Consent::Granted;
    queue(&dir).write(&report).unwrap();

    let sent = queue(&dir)
        .send_granted(|_| async { Err::<String, _>("Server hat mit 503 geantwortet") })
        .await;

    assert!(sent.is_empty());
    assert_eq!(queue(&dir).list(), vec![report]);
}

#[test]
fn message_is_capped_without_splitting_a_character() {
    let long = "ü".repeat(MESSAGE_MAX_BYTES);
    let report = ErrorReport::new(ReportKind::UiError, Phase::Idle, long, None, Vec::new());
    assert!(report.message.len() <= MESSAGE_MAX_BYTES);
    assert!(report.message.chars().all(|c| c == 'ü'));
}

#[test]
fn log_tail_keeps_the_newest_lines_within_the_cap() {
    let line = "x".repeat(1000);
    let lines: Vec<String> = (0..200).map(|i| format!("{i:03} {line}")).collect();
    let capped = cap_lines(lines, LOG_TAIL_MAX_BYTES);
    assert!(capped.iter().map(|l| l.len() + 1).sum::<usize>() <= LOG_TAIL_MAX_BYTES);
    assert!(capped.last().unwrap().starts_with("199 "));
}

#[test]
fn ring_keeps_the_last_three_hundred_lines_in_order() {
    let _guard = global_state_guard();
    clear_ring();
    for i in 0..350 {
        push_ring_line(&format!("line {i}"));
    }

    let lines = ring_tail();
    assert_eq!(lines.len(), RING_CAPACITY);
    assert_eq!(lines.first().unwrap(), "line 50");
    assert_eq!(lines.last().unwrap(), "line 349");
}

#[test]
fn ring_holds_fewer_lines_than_its_capacity_without_padding() {
    let _guard = global_state_guard();
    clear_ring();
    push_ring_line("nur eine Zeile");
    assert_eq!(ring_tail(), vec!["nur eine Zeile".to_string()]);
}

/// The ring as a report carries it. Read through the building step because
/// that is the only reader the ring has.
fn ring_tail() -> Vec<String> {
    super::collect::report(ReportKind::UiError, Phase::Idle, "", None).log_tail
}

// -- Mode and the panic hook (D1, D5) --

#[test]
fn the_installation_override_beats_the_setting() {
    assert_eq!(resolve_mode(Some("always"), true), ReportMode::Never);
    assert_eq!(resolve_mode(Some("ask"), true), ReportMode::Never);
}

#[test]
fn an_unknown_setting_value_falls_back_to_ask() {
    assert_eq!(resolve_mode(Some("vielleicht"), false), ReportMode::Ask);
    assert_eq!(resolve_mode(None, false), ReportMode::Ask);
    assert_eq!(resolve_mode(Some("always"), false), ReportMode::Always);
    assert_eq!(resolve_mode(Some("never"), false), ReportMode::Never);
}

#[test]
fn a_panic_on_another_thread_leaves_a_report_with_its_location() {
    let _guard = global_state_guard();
    let dir = tempdir().unwrap();
    set_active_reports_dir(dir.path().to_path_buf());
    set_mode(ReportMode::Ask);
    set_phase(Phase::Recording);
    set_context(ReportContext {
        settings: ReportSettings::default(),
        output_dir: Some("/Users/anna/Documents/cogniclone Workflows".to_string()),
    });
    clear_ring();
    push_ring_line("Recording started: /Users/anna/Documents/cogniclone Workflows/Urlaub 2026");

    let previous = std::panic::take_hook();
    install_panic_hook();
    // The panic is deliberate; its message must not drown the test output.
    let handle = std::thread::spawn(|| {
        panic!("Bildschirmfoto konnte nicht geschrieben werden");
    });
    assert!(handle.join().is_err());
    std::panic::set_hook(previous);

    // Selected by message rather than by taking the only file: the hook is
    // process-global while it is installed, so an assertion failure in a
    // test running on another thread would also land a report here. That
    // would turn one failing test into two.
    let reports: Vec<_> = queue(&dir)
        .list()
        .into_iter()
        .filter(|r| r.message.contains("Bildschirmfoto"))
        .collect();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.kind, ReportKind::Panic);
    assert_eq!(report.phase, Phase::Recording);
    assert_eq!(report.consent, Consent::Pending);
    assert!(report.message.contains("Bildschirmfoto"));
    // The panic path scrubs the ring like every other path does.
    assert!(report
        .log_tail
        .iter()
        .any(|l| l.contains("<Anleitungsverzeichnis>/<Aufnahme>")));
    assert!(!report.log_tail.iter().any(|l| l.contains("Urlaub 2026")));
    let location = report.location.as_deref().expect("panic location");
    // The panic! above is in this file, wherever the test module lives.
    assert!(
        location
            .replace('\\', "/")
            .contains("error_reports/tests.rs"),
        "location was {location}"
    );
}

#[test]
fn the_hook_is_not_installed_when_reports_are_off() {
    let _guard = global_state_guard();
    let dir = tempdir().unwrap();
    set_active_reports_dir(dir.path().to_path_buf());
    set_mode(ReportMode::Never);

    let previous = std::panic::take_hook();
    install_panic_hook();
    let handle = std::thread::spawn(|| panic!("nichts davon wird gemeldet"));
    assert!(handle.join().is_err());
    std::panic::set_hook(previous);
    set_mode(ReportMode::Ask);

    assert!(
        !queue(&dir)
            .list()
            .iter()
            .any(|r| r.message.contains("nichts davon wird gemeldet")),
        "mode never must not write a report for a panic it saw",
    );
}

// -- Consent and the queue (D7) --

#[test]
fn a_decline_removes_the_file() {
    let dir = tempdir().unwrap();
    let report = sample();
    queue(&dir).write(&report).unwrap();

    assert!(queue(&dir).decide(&report.report_id, false, None).is_none());
    assert!(queue(&dir).list().is_empty());
}

#[test]
fn a_grant_records_the_comment_and_keeps_the_file() {
    let dir = tempdir().unwrap();
    let mut report = sample();
    report.comment = None;
    queue(&dir).write(&report).unwrap();

    let granted = queue(&dir)
        .decide(
            &report.report_id,
            true,
            Some("  Ich wollte die Aufnahme stoppen  ".to_string()),
        )
        .unwrap();
    assert_eq!(granted.consent, Consent::Granted);
    assert_eq!(
        granted.comment.as_deref(),
        Some("Ich wollte die Aufnahme stoppen")
    );

    let on_disk = &queue(&dir).list()[0];
    assert_eq!(on_disk.consent, Consent::Granted);
    assert_eq!(on_disk.comment, granted.comment);
}

#[test]
fn a_grant_without_a_comment_carries_none() {
    let dir = tempdir().unwrap();
    let mut report = sample();
    report.comment = None;
    queue(&dir).write(&report).unwrap();

    let granted = queue(&dir)
        .decide(&report.report_id, true, Some("   ".to_string()))
        .unwrap();
    assert_eq!(granted.comment, None);
}

#[test]
fn the_sweep_removes_a_report_older_than_thirty_days() {
    let dir = tempdir().unwrap();
    let now = chrono::Utc::now();

    let mut stale = sample();
    stale.occurred_at = (now - chrono::Duration::days(31)).to_rfc3339();
    let mut fresh = sample();
    fresh.report_id = uuid::Uuid::new_v4().to_string();
    fresh.occurred_at = (now - chrono::Duration::days(29)).to_rfc3339();
    queue(&dir).write(&stale).unwrap();
    queue(&dir).write(&fresh).unwrap();

    assert_eq!(queue(&dir).sweep_stale(now), 1);
    let remaining = queue(&dir).list();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].report_id, fresh.report_id);
}

#[test]
fn nothing_is_written_when_reports_are_off() {
    let _guard = global_state_guard();
    let dir = tempdir().unwrap();
    set_active_reports_dir(dir.path().to_path_buf());
    set_mode(ReportMode::Never);

    assert!(create(ReportKind::CommandError, Phase::Idle, "Upload failed", None).is_none());
    assert!(queue(&dir).list().is_empty());
    set_mode(ReportMode::Ask);
}

#[test]
fn a_created_report_carries_the_scrubbed_ring_and_the_job_id() {
    let _guard = global_state_guard();
    let dir = tempdir().unwrap();
    set_active_reports_dir(dir.path().to_path_buf());
    set_mode(ReportMode::Ask);
    set_context(ReportContext {
        settings: ReportSettings {
            upload_target: Some("Staging".to_string()),
            pipeline_version: 2,
            generation_model: "azure/gpt-5.4".to_string(),
            hide_from_screenshots: true,
            skip_pii_check: false,
        },
        output_dir: Some("/Users/anna/Documents/cogniclone Workflows".to_string()),
    });
    clear_ring();
    push_ring_line("Recording started: /Users/anna/Documents/cogniclone Workflows/Urlaub 2026");

    let report = create(
        ReportKind::CommandError,
        Phase::Processing,
        "Upload failed: 500 - keine Antwort",
        Some("job-7".to_string()),
    )
    .unwrap();

    assert_eq!(report.kind, ReportKind::CommandError);
    assert_eq!(report.job_id.as_deref(), Some("job-7"));
    assert_eq!(report.settings.unwrap().pipeline_version, 2);
    assert_eq!(report.log_tail.len(), 1);
    assert!(!report.log_tail[0].contains("Urlaub 2026"));
    assert!(report.log_tail[0].contains("<Anleitungsverzeichnis>/<Aufnahme>"));
    assert_eq!(queue(&dir).list().len(), 1);
}
