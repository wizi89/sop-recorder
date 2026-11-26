//! The on-disk queue (D7): every report waits here, as one JSON file, until
//! the user has decided and a session exists to send it with.

use serde::Serialize;
use std::future::Future;
use std::path::PathBuf;

use super::state::{active_reports_dir, APP_HANDLE};
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

/// A report the queue handed to a sender that accepted it, with the number
/// the server answered. Serialised as is for the notice the webview shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sent {
    pub report_id: String,
    pub number: String,
}

/// The reports waiting in one directory, one `<report id>.json` file each.
///
/// The app has one queue, `Queue::active()`; tests open one on a temp
/// directory. The file name is decided here and nowhere else, so callers
/// speak in report ids.
pub struct Queue {
    dir: PathBuf,
}

impl Queue {
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The queue the app writes to and reads from.
    pub fn active() -> Option<Self> {
        active_reports_dir().map(Self::at)
    }

    /// Where a report's file is, whether or not it exists. `None` for an id
    /// that is not the canonical UUID `ErrorReport::new` gives every report.
    ///
    /// Ids arrive from the webview, and this is the one place an id becomes a
    /// path, so the check lives here: an id such as `../x` would otherwise
    /// name a file outside the queue for `get`, `decide` or the path command.
    pub fn path_of(&self, report_id: &str) -> Option<PathBuf> {
        let canonical = uuid::Uuid::parse_str(report_id)
            .is_ok_and(|id| id.hyphenated().to_string() == report_id);
        canonical.then(|| self.dir.join(format!("{}.json", report_id)))
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
    pub fn write(&self, report: &ErrorReport) -> Result<PathBuf, std::io::Error> {
        let path = self.persist(report)?;
        announce_created(report);
        Ok(path)
    }

    /// The only function that puts a report on disk. Everything that creates
    /// one goes through `write`, which adds the announcement; `decide` uses
    /// this directly, because recording a consent is not the creation of a
    /// report and must not tell every window that a new one has arrived.
    fn persist(&self, report: &ErrorReport) -> Result<PathBuf, std::io::Error> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path_of(&report.report_id).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a report id")
        })?;
        let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn get(&self, report_id: &str) -> Option<ErrorReport> {
        read(&self.path_of(report_id)?)
    }

    /// Every report still on disk, oldest first. A file that does not parse is
    /// skipped rather than failing the listing: one unreadable report must not
    /// hide the rest.
    pub fn list(&self) -> Vec<ErrorReport> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut reports: Vec<ErrorReport> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .filter_map(|p| read(&p))
            .collect();
        reports.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));
        reports
    }

    fn remove(&self, report_id: &str) {
        if let Some(path) = self.path_of(report_id) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Record the user's decision (D7). Granting keeps the file with the
    /// comment attached; declining deletes it, because a declined report is
    /// not something to keep a record of. Returns the granted report, so the
    /// caller can submit it without reading the file back.
    pub fn decide(
        &self,
        report_id: &str,
        grant: bool,
        comment: Option<String>,
    ) -> Option<ErrorReport> {
        let mut report = self.get(report_id)?;
        if !grant {
            self.remove(report_id);
            return None;
        }
        report.consent = Consent::Granted;
        report.comment = comment
            .map(|c| truncate_bytes(c.trim(), COMMENT_MAX_BYTES))
            .filter(|c| !c.is_empty());
        self.persist(&report).ok()?;
        Some(report)
    }

    /// Delete reports that have waited longer than `MAX_REPORT_AGE_DAYS`, sent
    /// or not. Returns how many went. A report whose timestamp does not parse
    /// is left alone: an unreadable date is not evidence of age.
    pub fn sweep_stale(&self, now: chrono::DateTime<chrono::Utc>) -> usize {
        let mut removed = 0;
        for report in self.list() {
            let Ok(occurred) = chrono::DateTime::parse_from_rfc3339(&report.occurred_at) else {
                continue;
            };
            if (now - occurred.with_timezone(&chrono::Utc)).num_days() > MAX_REPORT_AGE_DAYS {
                self.remove(&report.report_id);
                removed += 1;
            }
        }
        removed
    }

    /// Hand every granted report to `send`, oldest first (D7). Pending reports
    /// are never handed over: nothing leaves without consent.
    ///
    /// A report `send` accepts is deleted; one it refuses stays on disk, so
    /// the same report id goes out next time and the number the user was
    /// shown does not change. The one mechanism serves both the report created
    /// while signed out and the retry after a failed send.
    pub async fn send_granted<F, Fut, E>(&self, mut send: F) -> Vec<Sent>
    where
        F: FnMut(ErrorReport) -> Fut,
        Fut: Future<Output = Result<String, E>>,
        E: std::fmt::Display,
    {
        let mut sent = Vec::new();
        for report in self.list() {
            if report.consent != Consent::Granted {
                continue;
            }
            let report_id = report.report_id.clone();
            let short = report.number();
            match send(report).await {
                Ok(number) => {
                    self.remove(&report_id);
                    log::info!("Fehlerbericht {} gesendet", number);
                    sent.push(Sent { report_id, number });
                }
                Err(err) => {
                    // Kept on disk deliberately: the next sign-in retries it
                    // with the same id, so the number the user copied still
                    // finds it.
                    log::warn!("Fehlerbericht {} bleibt liegen: {}", short, err);
                }
            }
        }
        sent
    }
}

/// Tell every webview a report is waiting. Best effort by design: a report on
/// disk that nobody was told about is still found by the next `Queue::list`,
/// so a failed emit delays the dialog rather than losing the report.
fn announce_created(report: &ErrorReport) {
    if let Some(handle) = APP_HANDLE.get() {
        use tauri::Emitter;
        let _ = handle.emit(REPORT_CREATED_EVENT, report.report_id.clone());
    }
}

fn read(path: &std::path::Path) -> Option<ErrorReport> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// A report waits at most this long for a session to send it with (D7), so a
/// machine that never comes back online does not accumulate reports for ever.
pub const MAX_REPORT_AGE_DAYS: i64 = 30;

#[cfg(test)]
mod tests;
