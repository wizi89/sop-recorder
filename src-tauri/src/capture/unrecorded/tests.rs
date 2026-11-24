use super::*;

const HIGH: u32 = 0x3000;

fn app(name: &str) -> UnrecordedWindow {
    UnrecordedWindow::App(name.into())
}

#[test]
fn a_higher_level_is_unrecorded_and_an_equal_or_lower_one_is_not() {
    assert!(is_unrecorded(Some(HIGH), MEDIUM_LEVEL));
    assert!(!is_unrecorded(Some(MEDIUM_LEVEL), MEDIUM_LEVEL));
    assert!(!is_unrecorded(Some(0x1000), MEDIUM_LEVEL));
    // The recorder itself runs with the same rights: nothing is unrecorded.
    assert!(!is_unrecorded(Some(HIGH), HIGH));
}

#[test]
fn a_level_that_cannot_be_read_counts_as_unrecorded() {
    assert!(is_unrecorded(None, MEDIUM_LEVEL));
    assert!(is_unrecorded(None, HIGH));
}

#[test]
fn the_security_prompt_is_unrecorded_unless_the_recorder_runs_as_system() {
    assert!(is_unrecorded(Some(SYSTEM_LEVEL), HIGH));
    assert!(!is_unrecorded(Some(SYSTEM_LEVEL), SYSTEM_LEVEL));
}

#[test]
fn each_visit_is_written_when_it_ends() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::new(dir.path());
    assert!(!log.foreground(1.0, None));
    assert!(log.foreground(2.0, Some(UnrecordedWindow::SecurityPrompt)));
    // Nothing is written while the visit is still open.
    assert_eq!(read(dir.path()).unwrap(), vec![]);
    assert!(log.foreground(5.0, Some(app("Setup"))));
    // The same app coming to the front again continues the visit.
    assert!(log.foreground(6.0, Some(app("Setup"))));
    assert!(!log.foreground(9.5, None));
    assert_eq!(
        read(dir.path()).unwrap(),
        vec![
            Visit {
                window: UnrecordedWindow::SecurityPrompt,
                start_seconds: 2.0,
                end_seconds: 5.0
            },
            Visit {
                window: app("Setup"),
                start_seconds: 5.0,
                end_seconds: 9.5
            },
        ]
    );
}

#[test]
fn the_file_names_apps_but_no_window_title() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = Log::new(dir.path());
    log.foreground(1.0, Some(app("regedit")));
    log.foreground(3.0, Some(UnrecordedWindow::UnknownApp));
    log.foreground(4.0, None);
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join(FILE)).unwrap()).unwrap();
    assert_eq!(
        json,
        serde_json::json!([
            {"window": {"kind": "app", "name": "regedit"}, "start_seconds": 1.0, "end_seconds": 3.0},
            {"window": {"kind": "unknown_app"}, "start_seconds": 3.0, "end_seconds": 4.0},
        ])
    );
}

#[test]
fn a_recording_without_unrecorded_windows_reads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read(dir.path()).unwrap(), vec![]);
}

#[test]
fn an_unreadable_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(FILE), "not json").unwrap();
    assert!(read(dir.path()).is_err());
}
