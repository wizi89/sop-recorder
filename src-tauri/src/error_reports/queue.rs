//! The on-disk queue (D7): every report waits here, as one JSON file, until
//! the user has decided and a session exists to send it with.

use std::path::{Path, PathBuf};

use super::state::APP_HANDLE;
use super::{truncate_bytes, Consent, ErrorReport, COMMENT_MAX_BYTES};

/// The event the webview listens for. Carries the report id, not the report:
/// the webview reads the file back through `read_error_report`, so there is
/// one path by which report content reaches the dialog.
pub const REPORT_CREATED_EVENT: &str = "error_report:created";

/// Where reports wait. Resolved without an `AppHandle` so the panic hook,
/// which is installed before the Tauri builder runs (D5), can reach it.
pub fn reports_dir() -> Option<PathBuf> {
    dirs_next::data_local_dir().map(|p| p.join("com.cogniclone.recorder").join("error-reports"))
}

/// Write a report and announce it.
///
/// The announcement lives here, in the one function that writes, because it
/// was once only in the panic hook: `create` wrote the file and told nobody.
/// That went unnoticed because the only caller was the main window's own hook,
/// which calls `refresh` itself -- so a report created anywhere else, such as
/// from the settings window, landed on disk and no dialog ever opened. Tying
/// the two together makes writing without announcing impossible rather than
/// merely discouraged.
///
/// `Emitter::emit` reaches every webview, which is what makes a report raised
/// in one window open the dialog in another. There is no app handle during
/// tests or before the builder runs (D5), and then this is just a write.
pub fn write_report(dir: &Path, report: &ErrorReport) -> Result<PathBuf, std::io::Error> {
    let path = persist(dir, report)?;
    announce_created(report);
    Ok(path)
}

/// The only function that puts a report on disk. Everything that creates one
/// goes through `write_report`, which adds the announcement; `decide` uses this
/// directly, because recording a consent is not the creation of a report and
/// must not tell every window that a new one has arrived.
fn persist(dir: &Path, report: &ErrorReport) -> Result<PathBuf, std::io::Error> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(report.file_name());
    let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Tell every webview a report is waiting. Best effort by design: a report on
/// disk that nobody was told about is still found by the next `list_reports`,
/// so a failed emit delays the dialog rather than losing the report.
fn announce_created(report: &ErrorReport) {
    if let Some(handle) = APP_HANDLE.get() {
        use tauri::Emitter;
        let _ = handle.emit(REPORT_CREATED_EVENT, report.report_id.clone());
    }
}

pub fn read_report(path: &Path) -> Option<ErrorReport> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Every report still on disk, oldest first. A file that does not parse is
/// skipped rather than failing the listing: one unreadable report must not
/// hide the rest.
pub fn list_reports(dir: &Path) -> Vec<ErrorReport> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut reports: Vec<ErrorReport> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|p| read_report(&p))
        .collect();
    reports.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));
    reports
}

pub fn delete_report(dir: &Path, report_id: &str) {
    let _ = std::fs::remove_file(dir.join(format!("{}.json", report_id)));
}

/// A report waits at most this long for a session to send it with (D7), so a
/// machine that never comes back online does not accumulate reports for ever.
pub const MAX_REPORT_AGE_DAYS: i64 = 30;

/// Record the user's decision (D7). Granting keeps the file with the comment
/// attached; declining deletes it, because a declined report is not something
/// to keep a record of. Returns the granted report, so the caller can submit
/// it without reading the file back.
pub fn decide(
    dir: &Path,
    report_id: &str,
    grant: bool,
    comment: Option<String>,
) -> Option<ErrorReport> {
    let path = dir.join(format!("{}.json", report_id));
    let mut report = read_report(&path)?;
    if !grant {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    report.consent = Consent::Granted;
    report.comment = comment
        .map(|c| truncate_bytes(c.trim(), COMMENT_MAX_BYTES))
        .filter(|c| !c.is_empty());
    persist(dir, &report).ok()?;
    Some(report)
}

/// Delete reports that have waited longer than `MAX_REPORT_AGE_DAYS`, sent or
/// not. Returns how many went. A report whose timestamp does not parse is
/// left alone: an unreadable date is not evidence of age.
pub fn sweep_stale(dir: &Path, now: chrono::DateTime<chrono::Utc>) -> usize {
    let mut removed = 0;
    for report in list_reports(dir) {
        let Ok(occurred) = chrono::DateTime::parse_from_rfc3339(&report.occurred_at) else {
            continue;
        };
        if (now - occurred.with_timezone(&chrono::Utc)).num_days() > MAX_REPORT_AGE_DAYS {
            delete_report(dir, &report.report_id);
            removed += 1;
        }
    }
    removed
}
