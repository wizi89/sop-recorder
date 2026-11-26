use super::*;
use crate::error_reports::{Phase, ReportKind};
use tempfile::tempdir;

/// Ids come from the webview, so the queue must refuse any id that is not
/// one it could have made: anything else could name a file outside it.
#[test]
fn an_id_that_is_not_a_report_id_names_no_file() {
    let dir = tempdir().unwrap();
    let queue = Queue::at(dir.path().join("error-reports"));
    // A real report just outside the queue, so a decline that followed
    // `../x` would find it, parse it and delete it.
    let mut report = ErrorReport::new(ReportKind::Panic, Phase::Idle, "m".into(), None, vec![]);
    report.report_id = "x".into();
    let outside = dir.path().join("x.json");
    std::fs::write(&outside, serde_json::to_string(&report).unwrap()).unwrap();

    for id in [
        "../x",
        "..\\x",
        "",
        // A UUID, but not in the shape `ErrorReport::new` writes.
        "{67e55044-10b1-426f-9247-bb680e5fe0c8}",
        "67E55044-10B1-426F-9247-BB680E5FE0C8",
        "67e5504410b1426f9247bb680e5fe0c8",
    ] {
        assert_eq!(queue.path_of(id), None, "{id:?} was accepted");
        assert!(queue.get(id).is_none());
        assert!(queue.decide(id, false, None).is_none());
    }
    assert!(
        outside.exists(),
        "a decline removed a file outside the queue"
    );

    let id = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    assert_eq!(
        queue.path_of(id),
        Some(dir.path().join("error-reports").join(format!("{id}.json")))
    );
}
