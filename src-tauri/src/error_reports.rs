//! Error reports: what the recorder collects when something fails, and the
//! on-disk queue those reports wait in until the user has decided.
//!
//! Design references are to the backend's
//! `openspec/changes/recorder-error-reports/design.md`.
//!
//! - D3 fixes the report's shape and the fields that may never appear in one.
//!   The type below is that shape; the fields it lacks are the enforcement.
//!   There is no screenshot, audio, transcript, guide, email, token or output
//!   path field, so no code path can put one in a report by accident.
//! - D4 keeps the recent log in a ring buffer rather than reading the log file.
//! - D5 makes the panic hook a writer and nothing else.
//! - D7 keeps every report on disk until a session exists to send it with.
//!
//! The caps are part of the contract: the server validates the same numbers
//! (D9) and answers 422 to a report that exceeds them, so a recorder that
//! truncates differently would produce reports the server refuses.

//!
//! The module splits along those decisions: `collect` builds a report from the
//! ring (D4), `scrub` cleans every line that goes into one (D3), `state` holds
//! what the panic hook may read without a lock (D5), `panic_hook` is the hook
//! itself (D5), and `queue` is the on-disk queue (D7).

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

mod collect;
mod panic_hook;
mod queue;
mod scrub;
mod state;

#[cfg(test)]
pub(crate) use collect::{clear_ring, push_ring_line};
pub use collect::{create, ring_lines, ring_log_target, RingTarget, RING_CAPACITY};
pub use panic_hook::{install_panic_hook, TRAIL_FILE};
pub use queue::{
    decide, delete_report, list_reports, read_report, reports_dir, sweep_stale, write_report,
    MAX_REPORT_AGE_DAYS, REPORT_CREATED_EVENT,
};
pub use scrub::Scrubber;
pub use state::{
    active_reports_dir, context, mode, phase, resolve_mode, scrubber, set_active_reports_dir,
    set_app_handle, set_context, set_mode, set_phase, ReportContext, ReportMode,
};

/// Bumped only when a field changes meaning. The server pins this value.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// D3: the message is capped at 4 KB, the log tail at 64 KB, the comment at
/// 2 KB. Bytes, not characters -- the server measures the encoded body.
pub const MESSAGE_MAX_BYTES: usize = 4 * 1024;
pub const LOG_TAIL_MAX_BYTES: usize = 64 * 1024;
pub const COMMENT_MAX_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Panic,
    CommandError,
    UiError,
}

impl ReportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ReportKind::Panic => "panic",
            ReportKind::CommandError => "command_error",
            ReportKind::UiError => "ui_error",
        }
    }
}

/// What the app was doing when it failed. D3 fixes the vocabulary; the server
/// tags the event with it and groups command and UI errors by it (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Startup,
    Login,
    Idle,
    Recording,
    Review,
    Processing,
    Settings,
    Unknown,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Startup => "startup",
            Phase::Login => "login",
            Phase::Idle => "idle",
            Phase::Recording => "recording",
            Phase::Review => "review",
            Phase::Processing => "processing",
            Phase::Settings => "settings",
            Phase::Unknown => "unknown",
        }
    }

    pub fn from_str_or_unknown(value: &str) -> Self {
        match value {
            "startup" => Phase::Startup,
            "login" => Phase::Login,
            "idle" => Phase::Idle,
            "recording" => Phase::Recording,
            "review" => Phase::Review,
            "processing" => Phase::Processing,
            "settings" => Phase::Settings,
            _ => Phase::Unknown,
        }
    }
}

/// D3: the handful of settings that decide which code path ran. Deliberately
/// not the whole `AppSettings` -- that carries the output directory, and the
/// output directory is one of the things a report may never contain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSettings {
    pub upload_target: Option<String>,
    pub pipeline_version: u8,
    pub generation_model: String,
    pub hide_from_screenshots: bool,
    pub skip_pii_check: bool,
}

/// Where a report stands with the user. A report is written before it is
/// shown (D5, D7), so `Pending` is the state every report starts in; a
/// decline deletes the file rather than recording a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Consent {
    Pending,
    Granted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReport {
    pub schema_version: u32,
    pub report_id: String,
    pub kind: ReportKind,
    pub occurred_at: String,
    pub app_version: String,
    pub os: String,
    pub os_version: String,
    pub arch: String,
    pub locale: String,
    pub phase: Phase,
    pub message: String,
    pub location: Option<String>,
    pub log_tail: Vec<String>,
    pub settings: Option<ReportSettings>,
    pub job_id: Option<String>,
    pub comment: Option<String>,
    /// Local bookkeeping only. Stripped before submission -- the server's
    /// schema has no such field, and a report only ever reaches it granted.
    pub consent: Consent,
}

impl ErrorReport {
    /// A new report, pending consent, with the caps already applied.
    pub fn new(
        kind: ReportKind,
        phase: Phase,
        message: String,
        location: Option<String>,
        log_tail: Vec<String>,
    ) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: uuid::Uuid::new_v4().to_string(),
            kind,
            occurred_at: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            os_version: os_version(),
            arch: std::env::consts::ARCH.to_string(),
            locale: locale(),
            phase,
            message: truncate_bytes(&message, MESSAGE_MAX_BYTES),
            location,
            log_tail: cap_lines(log_tail, LOG_TAIL_MAX_BYTES),
            settings: None,
            job_id: None,
            comment: None,
            consent: Consent::Pending,
        }
    }

    /// The eight characters the dialog shows the user and the tracker is
    /// searched by. The server derives the same value from the same id.
    pub fn number(&self) -> String {
        self.report_id.replace('-', "").chars().take(8).collect()
    }

    pub fn file_name(&self) -> String {
        format!("{}.json", self.report_id)
    }
}

/// Truncate to at most `max` bytes without splitting a character. German text
/// makes this real rather than theoretical: `ü` is two bytes.
pub fn truncate_bytes(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

/// Keep the newest lines that fit in `max` bytes. Oldest go first: the lines
/// nearest the failure are the ones that explain it.
pub fn cap_lines(lines: Vec<String>, max: usize) -> Vec<String> {
    let mut total = 0usize;
    let mut kept: Vec<String> = Vec::new();
    for line in lines.into_iter().rev() {
        let cost = line.len() + 1;
        if total + cost > max {
            break;
        }
        total += cost;
        kept.push(line);
    }
    kept.reverse();
    kept
}

fn locale() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                return value.split('.').next().unwrap_or(&value).to_string();
            }
        }
    }
    "unknown".to_string()
}

/// Probed once and cached. The panic hook builds a report, and a hook that
/// spawns a process on the panicking thread is doing exactly what D5 says it
/// must not; `install_panic_hook` warms this before any panic can happen.
fn os_version() -> String {
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED.get_or_init(probe_os_version).clone()
}

fn probe_os_version() -> String {
    // No new dependency for this: the platform's own version command is the
    // only source, and a report without it is still a usable report.
    #[cfg(target_os = "macos")]
    let probe = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output();
    #[cfg(target_os = "windows")]
    let probe = std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let probe = std::process::Command::new("uname").arg("-r").output();

    probe
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The ring buffer, the active reports directory, the mode and the phase are
/// process globals, so every test that sets one runs under this lock instead
/// of racing the others through cargo's thread pool. It lives at file scope
/// because the tests that need it are spread across several modules: keeping
/// it inside `mod tests` left `breadcrumb_isolation` racing everything else,
/// which passed for weeks and then failed once, locally, on an unrelated run.
///
/// Poisoning is ignored deliberately -- one panicking test must not cascade
/// into every later test failing on the lock instead of on its own assertion.
#[cfg(test)]
static GLOBAL_STATE_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
fn global_state_guard() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL_STATE_TESTS.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests;
