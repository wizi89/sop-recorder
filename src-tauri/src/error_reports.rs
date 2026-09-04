//! Error reports: what the recorder collects when something fails, and the
//! on-disk queue those reports wait in until the user has decided.
//!
//! Design references are to `openspec/changes/recorder-error-reports/design.md`
//! in the `sop-sorcery` repository.
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

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

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
    let probe = std::process::Command::new("sw_vers").arg("-productVersion").output();
    #[cfg(target_os = "windows")]
    let probe = std::process::Command::new("cmd").args(["/C", "ver"]).output();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let probe = std::process::Command::new("uname").arg("-r").output();

    probe
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

// -- The ring buffer (D4) --

/// How many formatted log lines the ring holds. 300 lines is roughly the last
/// half minute of a recording at the recorder's log volume, which is the span
/// that explains a failure; the whole buffer also has to fit the 64 KB cap on
/// `log_tail` after scrubbing.
pub const RING_CAPACITY: usize = 300;

static RING: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// A `log::Log` that keeps the last `RING_CAPACITY` formatted lines in memory.
///
/// It is the log target the panic hook reads (D5). Reading the tail of the log
/// *file* was rejected: the file's location differs between dev and release
/// builds, it can be 5 MB, and a panic hook that opens and seeks a file is
/// doing too much. This is in memory when the hook runs and costs nothing to
/// read.
///
/// The record reaching `log` has already been through the plugin's root
/// formatter, so `record.args()` is the finished line, timestamp and level
/// included -- the same text the log file gets.
pub struct RingTarget;

impl log::Log for RingTarget {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        // A poisoned ring must not take the process down with it, and must not
        // make logging itself panic inside a panic hook.
        let Ok(mut ring) = RING.lock() else { return };
        if ring.len() == RING_CAPACITY {
            ring.pop_front();
        }
        ring.push_back(record.args().to_string());
    }

    fn flush(&self) {}
}

static RING_TARGET: RingTarget = RingTarget;

/// The plugin target that feeds the ring.
///
/// Registration differs between the two builds and getting it wrong loses the
/// log file: the plugin's `target()` appends to its defaults, while `targets()`
/// replaces them. `lib.rs` uses `target()` on the release path, where it relies
/// on the defaults, and adds this to the list it already passes to `targets()`
/// on the dev path.
pub fn ring_log_target() -> tauri_plugin_log::Target {
    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Dispatch(
        tauri_plugin_log::fern::Dispatch::new().chain(&RING_TARGET as &'static dyn log::Log),
    ))
}

/// The lines currently in the ring, oldest first. Unscrubbed -- every caller
/// runs them through a `Scrubber` before they reach a report.
pub fn ring_lines() -> Vec<String> {
    RING.lock()
        .map(|ring| ring.iter().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn push_ring_line(line: &str) {
    log::Log::log(
        &RING_TARGET,
        &log::Record::builder()
            .args(format_args!("{}", line))
            .level(log::Level::Info)
            .build(),
    );
}

#[cfg(test)]
pub(crate) fn clear_ring() {
    if let Ok(mut ring) = RING.lock() {
        ring.clear();
    }
}

// -- Scrubbing (D3) --

/// Rewrites a log line so it can go into a report.
///
/// The log tail is the one field that could smuggle in something a report may
/// never contain: log lines carry recording directory paths, which embed both
/// the home directory and the guide title, and server error bodies, which can
/// carry a token. Every rule here removes one of those.
///
/// Scrubbing runs at report creation, before the file is written, so the copy
/// on disk is already clean and the content the dialog shows is exactly the
/// content that is sent.
///
/// What deliberately survives: cursor coordinates, the audio device name, and
/// HTTP status lines. They are not personal data and they are frequently the
/// diagnosis.
///
/// Adding a log line that names a file means adding a case here and a test for
/// it. That is the review rule, not a suggestion -- a line whose shape no rule
/// matches goes out verbatim.
pub struct Scrubber {
    home: Option<String>,
    output_dir: Option<String>,
}

impl Scrubber {
    pub fn new(home: Option<PathBuf>, output_dir: Option<String>) -> Self {
        Self {
            home: home
                .map(|p| p.to_string_lossy().trim_end_matches(['/', '\\']).to_string())
                .filter(|s| !s.is_empty()),
            output_dir: output_dir
                .map(|s| s.trim_end_matches(['/', '\\']).to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    pub fn scrub(&self, line: &str) -> String {
        // Output directory first: it usually sits under the home directory, so
        // rewriting the home prefix first would leave nothing for this rule to
        // match and the guide title would survive.
        let mut out = self.scrub_output_dir(line);
        if let Some(home) = &self.home {
            out = out.replace(home, "~");
        }
        out = scrub_emails(&out);
        out = scrub_secrets(&out);
        out
    }

    pub fn scrub_all(&self, lines: &[String]) -> Vec<String> {
        lines.iter().map(|l| self.scrub(l)).collect()
    }

    /// `<output dir>/<recording>/...` becomes
    /// `<Anleitungsverzeichnis>/<Aufnahme>/...`. The path component after the
    /// output directory is the recording folder, and its name is the guide
    /// title the user typed.
    fn scrub_output_dir(&self, line: &str) -> String {
        let Some(dir) = &self.output_dir else {
            return line.to_string();
        };
        let mut out = String::with_capacity(line.len());
        let mut rest = line;
        while let Some(at) = rest.find(dir.as_str()) {
            out.push_str(&rest[..at]);
            out.push_str("<Anleitungsverzeichnis>");
            let after = &rest[at + dir.len()..];
            let mut chars = after.char_indices();
            match chars.next() {
                // A separator means a path component follows, and that
                // component is the recording name.
                Some((_, sep)) if sep == '/' || sep == '\\' => {
                    out.push(sep);
                    let tail = &after[sep.len_utf8()..];
                    let end = tail
                        .find(['/', '\\', ' ', '"', '\''])
                        .unwrap_or(tail.len());
                    if end > 0 {
                        out.push_str("<Aufnahme>");
                    }
                    rest = &tail[end..];
                }
                _ => rest = after,
            }
        }
        out.push_str(rest);
        out
    }
}

/// Anything shaped like an address. Kept deliberately loose: a false positive
/// costs a placeholder in a log line, a false negative costs an address.
fn scrub_emails(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for token in split_keeping_separators(line) {
        if looks_like_email(token) {
            out.push_str("<E-Mail>");
        } else {
            out.push_str(token);
        }
    }
    out
}

fn looks_like_email(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| !c.is_alphanumeric());
    let Some((local, domain)) = trimmed.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

fn split_keeping_separators(line: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in line.char_indices() {
        if c.is_whitespace() || c == '"' || c == ',' || c == '<' || c == '>' {
            if i > start {
                parts.push(&line[start..i]);
            }
            parts.push(&line[i..i + c.len_utf8()]);
            start = i + c.len_utf8();
        }
    }
    if start < line.len() {
        parts.push(&line[start..]);
    }
    parts
}

/// `Bearer <token>`, and the values of the JSON keys that carry credentials.
fn scrub_secrets(line: &str) -> String {
    let mut out = replace_after_marker(line, "Bearer ", |c| c.is_whitespace() || c == '"');
    for key in ["access_token", "refresh_token", "api_key"] {
        for marker in [format!("\"{key}\":\""), format!("\"{key}\": \"")] {
            out = replace_after_marker(&out, &marker, |c| c == '"');
        }
    }
    out
}

/// Replace everything between `marker` and the first character `ends` accepts
/// with `<entfernt>`, for every occurrence.
fn replace_after_marker(line: &str, marker: &str, ends: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(marker) {
        out.push_str(&rest[..at + marker.len()]);
        let value = &rest[at + marker.len()..];
        let end = value.find(&ends).unwrap_or(value.len());
        if end > 0 {
            out.push_str("<entfernt>");
        }
        rest = &value[end..];
    }
    out.push_str(rest);
    out
}

// -- Mode, phase, and the ambient facts a report is built from --

/// The three values of the `error_reports` setting (D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    Ask,
    Always,
    Never,
}

impl ReportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ReportMode::Ask => "ask",
            ReportMode::Always => "always",
            ReportMode::Never => "never",
        }
    }
}

/// The mode that actually applies, given the saved setting and the
/// installation-wide override (D1). An installation that switches reports off
/// wins over anything the user chose, and an unrecognised setting value falls
/// back to the default rather than to a mode the user never picked.
pub fn resolve_mode(setting: Option<&str>, forced_off: bool) -> ReportMode {
    if forced_off {
        return ReportMode::Never;
    }
    match setting {
        Some("always") => ReportMode::Always,
        Some("never") => ReportMode::Never,
        _ => ReportMode::Ask,
    }
}

const MODE_ASK: u8 = 0;
const MODE_ALWAYS: u8 = 1;
const MODE_NEVER: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(MODE_ASK);

/// Atomics rather than a lock, throughout this section: every value here is
/// read by the panic hook, which runs on the panicking thread and may be
/// holding, or racing, any lock in the process (D5).
pub fn set_mode(mode: ReportMode) {
    MODE.store(
        match mode {
            ReportMode::Ask => MODE_ASK,
            ReportMode::Always => MODE_ALWAYS,
            ReportMode::Never => MODE_NEVER,
        },
        Ordering::Relaxed,
    );
}

pub fn mode() -> ReportMode {
    match MODE.load(Ordering::Relaxed) {
        MODE_ALWAYS => ReportMode::Always,
        MODE_NEVER => ReportMode::Never,
        _ => ReportMode::Ask,
    }
}

static PHASE: AtomicU8 = AtomicU8::new(0);

/// What the app is doing now, for a report the UI cannot annotate itself --
/// which means a panic. Reports the webview creates carry the phase the
/// webview knows, which is finer.
pub fn set_phase(phase: Phase) {
    PHASE.store(
        match phase {
            Phase::Startup => 0,
            Phase::Login => 1,
            Phase::Idle => 2,
            Phase::Recording => 3,
            Phase::Review => 4,
            Phase::Processing => 5,
            Phase::Settings => 6,
            Phase::Unknown => 7,
        },
        Ordering::Relaxed,
    );
}

pub fn phase() -> Phase {
    match PHASE.load(Ordering::Relaxed) {
        0 => Phase::Startup,
        1 => Phase::Login,
        2 => Phase::Idle,
        3 => Phase::Recording,
        4 => Phase::Review,
        5 => Phase::Processing,
        6 => Phase::Settings,
        _ => Phase::Unknown,
    }
}

/// The settings subset and the scrubbing inputs, kept as a snapshot so a
/// report can be built without touching the settings store. The panic hook
/// cannot go through the store: it runs on the panicking thread, and the store
/// takes a lock.
static SNAPSHOT: Mutex<Option<ReportContext>> = Mutex::new(None);

#[derive(Debug, Clone, Default)]
pub struct ReportContext {
    pub settings: ReportSettings,
    pub output_dir: Option<String>,
}

pub fn set_context(context: ReportContext) {
    if let Ok(mut slot) = SNAPSHOT.lock() {
        *slot = Some(context);
    }
}

/// `try_lock`, not `lock`: a report without the settings subset is still a
/// report, and a panic hook that blocks on a poisoned or held lock is a hang
/// on the way out of the process.
pub fn context() -> Option<ReportContext> {
    SNAPSHOT.try_lock().ok().and_then(|slot| slot.clone())
}

pub fn scrubber() -> Scrubber {
    Scrubber::new(
        dirs_next::home_dir(),
        context().and_then(|c| c.output_dir),
    )
}

/// Where reports are written. Set once at startup from `reports_dir()`; the
/// tests point it at a temp directory.
static ACTIVE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_active_reports_dir(dir: PathBuf) {
    if let Ok(mut slot) = ACTIVE_DIR.lock() {
        *slot = Some(dir);
    }
}

pub fn active_reports_dir() -> Option<PathBuf> {
    ACTIVE_DIR
        .try_lock()
        .ok()
        .and_then(|slot| slot.clone())
        .or_else(reports_dir)
}

/// Set in `setup()` so the hook can tell the main window a report appeared.
/// A thread panic leaves the process running, and the dialog should not wait
/// for a relaunch that may never come.
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// The event the webview listens for. Carries the report id, not the report:
/// the webview reads the file back through `read_error_report`, so there is
/// one path by which report content reaches the dialog.
pub const REPORT_CREATED_EVENT: &str = "error_report:created";

/// The lock-free record of what the panic hook managed to do. Not a report:
/// it carries no user content, only stage names and timestamps.
pub const TRAIL_FILE: &str = "panic-trail.log";
const TRAIL_MAX_BYTES: u64 = 64 * 1024;

// -- The panic hook (D5) --

/// Install the hook that turns a panic into a report on disk.
///
/// Called before the Tauri builder runs, so it also covers a panic during
/// startup. It writes a file and calls the previous hook; it shows no UI and
/// makes no network request, because a panic hook runs on the panicking
/// thread, possibly while a lock it needs is poisoned and possibly while the
/// process is about to end. Everything else -- asking the user, sending --
/// happens later, from the webview, off the back of the file.
///
/// Not installed at all when reports are off at startup. Switching the setting
/// to `never` mid-session also stops it, because the hook re-reads the mode;
/// switching it *on* mid-session takes effect for command and UI errors at
/// once and for panics after the next launch.
pub fn install_panic_hook() {
    if mode() == ReportMode::Never {
        breadcrumb("hook not installed: mode is never");
        return;
    }
    // Warm the cache while a subprocess is still a safe thing to spawn.
    let _ = os_version();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write_panic_report(info);
        previous(info);
    }));
    breadcrumb("hook installed");
}

/// A one-line record that a panic happened, written before anything that can
/// fail is touched.
///
/// The full report needs the ring buffer, the scrubber, the settings snapshot
/// and a serialiser -- four things that take locks or allocate, inside a hook
/// running on a thread that is already unwinding. If any of them misbehaves,
/// the crash leaves no trace at all, which is how a main-thread panic came to
/// kill the app and produce nothing. This uses only `std::fs` and takes no
/// lock, so it records the crash even when the reporting machinery cannot, and
/// it says which stage was reached.
fn breadcrumb(stage: &str) {
    // Next to the active reports directory, not to `data_local_dir` directly.
    // The panic-hook tests install the process-global hook and panic on
    // purpose; addressing the real directory meant `cargo test` appended to
    // the user's own trail and wrote lines that read exactly like a misbehaving
    // app -- three installs in nine seconds and a "mode is never" that no app
    // run produced. A diagnostic that fabricates evidence is worse than none.
    let Some(dir) = active_reports_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let line = format!(
        "{} {}\n",
        chrono::Utc::now().to_rfc3339(),
        stage,
    );
    // One line per launch plus four per panic, appended forever, in a
    // directory that otherwise empties itself. Start over rather than grow
    // without limit; the recent past is the only part anyone reads.
    let path = dir.join(TRAIL_FILE);
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len() > TRAIL_MAX_BYTES)
        .unwrap_or(false);

    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(too_big)
        .truncate(too_big)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn write_panic_report(info: &std::panic::PanicHookInfo<'_>) {
    breadcrumb("panic-hook-entered");
    if mode() == ReportMode::Never {
        breadcrumb("stopped: mode is never");
        return;
    }
    let Some(dir) = active_reports_dir() else {
        breadcrumb("stopped: no reports directory");
        return;
    };

    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_string());
    let location = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()));

    // The message and the location, not a stack trace: release binaries ship
    // stripped, deliberately, and a backtrace from one is a list of addresses
    // nothing can symbolicate (D8).
    let scrubber = scrubber();
    let mut report = ErrorReport::new(
        ReportKind::Panic,
        phase(),
        scrubber.scrub(&payload),
        location,
        scrubber.scrub_all(&ring_lines()),
    );
    report.settings = context().map(|c| c.settings);

    breadcrumb("collected, about to write");
    // `write_report` announces it; see the note there on why that is not done
    // separately any more.
    match write_report(&dir, &report) {
        Ok(_) => breadcrumb(&format!("wrote report {}", report.report_id)),
        Err(e) => breadcrumb(&format!("stopped: write failed: {}", e)),
    }
}

// -- The on-disk queue (D7) --

/// Where reports wait. Resolved without an `AppHandle` so the panic hook,
/// which is installed before the Tauri builder runs (D5), can reach it.
pub fn reports_dir() -> Option<PathBuf> {
    dirs_next::data_local_dir()
        .map(|p| p.join("com.cogniclone.recorder").join("error-reports"))
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
    let json = serde_json::to_string_pretty(report)
        .map_err(std::io::Error::other)?;
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

/// Build a report for a failure the webview saw: a command that returned an
/// error it could not explain, or an unhandled error in the webview itself.
/// The Rust side attaches the log tail and the platform facts, so those are
/// collected in one place for all three kinds.
pub fn create(
    kind: ReportKind,
    phase: Phase,
    message: &str,
    job_id: Option<String>,
) -> Option<ErrorReport> {
    if mode() == ReportMode::Never {
        return None;
    }
    let dir = active_reports_dir()?;
    let scrubber = scrubber();
    let mut report = ErrorReport::new(
        kind,
        phase,
        scrubber.scrub(message),
        None,
        scrubber.scrub_all(&ring_lines()),
    );
    report.settings = context().map(|c| c.settings);
    report.job_id = job_id;
    write_report(&dir, &report).ok()?;
    Some(report)
}

/// Every path that puts a report on disk goes through `write_report`, which is
/// what makes the dialog appear no matter which window raised the failure.
///
/// Asserted rather than trusted: the announcement used to sit in the panic
/// hook alone, so a report created from the settings window was written and
/// never shown. A second `std::fs::write` added elsewhere would reintroduce
/// exactly that, and it would look fine in review.
#[cfg(test)]
mod single_writer {
    #[test]
    fn only_write_report_writes_a_report_file() {
        let source = include_str!("error_reports.rs");
        // Only the shipping half of the file: test fixtures write their own
        // files on purpose, and this very test names the call it looks for.
        // Split on the test *modules*, not on `#[cfg(test)]` -- that also
        // decorates individual helpers much earlier in the file, and cutting
        // there would hide `write_report` itself and pass for the wrong reason.
        let production = source.split("#[cfg(test)]\nmod ").next().unwrap();
        // Assembled at runtime so the needle is not itself a match.
        let needle = format!("std::fs::{}(", "write");
        let writes = production.matches(&needle).count();
        assert_eq!(
            writes, 1,
            "a report file is written somewhere other than `persist`; every \
             creation must go through `write_report`, which announces it, or \
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
#[cfg(test)]
mod debug_triggers {
    const SOURCE: &str = include_str!("commands/error_reports.rs");

    fn arm(kind: &str) -> &'static str {
        let start = SOURCE
            .find(&format!("\"{}\" =>", kind))
            .unwrap_or_else(|| panic!("no `{}` arm in debug_trigger_failure", kind));
        let rest = &SOURCE[start..];
        let end = rest.find("\n            \"").unwrap_or(rest.len());
        &rest[..end]
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
#[cfg(test)]
mod breadcrumb_isolation {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn the_trail_starts_over_instead_of_growing_without_limit() {
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
        let dir = tempdir().unwrap();
        set_active_reports_dir(dir.path().to_path_buf());

        breadcrumb("test marker");

        let trail = dir.path().join("panic-trail.log");
        assert!(trail.exists(), "the trail must land in the active directory");
        assert!(std::fs::read_to_string(&trail).unwrap().contains("test marker"));

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
        let path = write_report(dir.path(), &report).unwrap();

        let read_back = read_report(&path).unwrap();
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
        write_report(dir.path(), &older).unwrap();
        write_report(dir.path(), &newer).unwrap();
        std::fs::write(dir.path().join("broken.json"), "{ not json").unwrap();

        let listed = list_reports(dir.path());
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].report_id, older.report_id);
        assert_eq!(listed[1].report_id, newer.report_id);
    }

    #[test]
    fn delete_removes_the_file() {
        let dir = tempdir().unwrap();
        let report = sample();
        write_report(dir.path(), &report).unwrap();
        delete_report(dir.path(), &report.report_id);
        assert!(list_reports(dir.path()).is_empty());
    }

    #[test]
    fn message_is_capped_without_splitting_a_character() {
        let long = "ü".repeat(MESSAGE_MAX_BYTES);
        let report = ErrorReport::new(
            ReportKind::UiError,
            Phase::Idle,
            long,
            None,
            Vec::new(),
        );
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

    /// The ring is global, so the two tests that use it run under one lock
    /// rather than racing each other through cargo's thread pool.
    static RING_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn ring_keeps_the_last_three_hundred_lines_in_order() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_ring();
        for i in 0..350 {
            push_ring_line(&format!("line {i}"));
        }

        let lines = ring_lines();
        assert_eq!(lines.len(), RING_CAPACITY);
        assert_eq!(lines.first().unwrap(), "line 50");
        assert_eq!(lines.last().unwrap(), "line 349");
    }

    #[test]
    fn ring_holds_fewer_lines_than_its_capacity_without_padding() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_ring();
        push_ring_line("nur eine Zeile");
        assert_eq!(ring_lines(), vec!["nur eine Zeile".to_string()]);
    }

    // -- Scrubbing (D3). One test per rule, each fed a log line shape that
    // exists in the codebase today.

    fn scrubber() -> Scrubber {
        Scrubber::new(
            Some(PathBuf::from("/Users/anna")),
            Some("/Users/anna/Documents/cogniclone Workflows".to_string()),
        )
    }

    #[test]
    fn recording_directory_loses_the_guide_title() {
        // commands/recording.rs:239
        let line = "[2026-09-03][12:00:01][INFO] Recording started: \
/Users/anna/Documents/cogniclone Workflows/Urlaubsantrag stellen 2026-09-03 12-00-00";
        let out = scrubber().scrub(line);
        assert!(!out.contains("Urlaubsantrag"));
        assert!(!out.contains("/Users/anna"));
        assert!(out.contains("<Anleitungsverzeichnis>/<Aufnahme>"));
    }

    #[test]
    fn home_directory_outside_the_output_dir_becomes_a_tilde() {
        // capture/audio.rs:198, when the output dir has been moved elsewhere.
        let line = "Audio recording started: /Users/anna/Library/Caches/rec.wav";
        let out = scrubber().scrub(line);
        assert_eq!(
            out,
            "Audio recording started: ~/Library/Caches/rec.wav"
        );
    }

    #[test]
    fn an_email_address_becomes_a_placeholder() {
        let line = "Anmeldung fehlgeschlagen für anna.mueller@kunde-gmbh.de";
        let out = scrubber().scrub(line);
        assert!(!out.contains("anna.mueller"));
        assert!(out.contains("<E-Mail>"));
    }

    #[test]
    fn a_bearer_token_is_removed() {
        let line = "GET /quota Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abc";
        let out = scrubber().scrub(line);
        assert!(!out.contains("eyJhbGci"));
        assert!(out.contains("Bearer <entfernt>"));
    }

    #[test]
    fn token_values_in_an_upload_error_body_are_removed() {
        // network/upload.rs:110
        let line = "Upload failed: 401 Unauthorized - \
{\"access_token\":\"eyJhbGciOi.secret\",\"refresh_token\": \"rt_9f3c\",\"error\":\"expired\"}";
        let out = scrubber().scrub(line);
        assert!(!out.contains("eyJhbGciOi.secret"));
        assert!(!out.contains("rt_9f3c"));
        assert!(out.contains("\"access_token\":\"<entfernt>\""));
        assert!(out.contains("\"refresh_token\": \"<entfernt>\""));
        // The status line and the server's error code are the diagnosis.
        assert!(out.contains("401 Unauthorized"));
        assert!(out.contains("\"error\":\"expired\""));
    }

    #[test]
    fn an_api_key_value_is_removed() {
        let line = "settings saved {\"api_key\":\"sk-proj-abcdef123456\"}";
        let out = scrubber().scrub(line);
        assert!(!out.contains("sk-proj"));
        assert!(out.contains("\"api_key\":\"<entfernt>\""));
    }

    #[test]
    fn cursor_coordinates_survive_scrubbing() {
        let line = "[2026-09-03][12:00:02][INFO] Click at (1284, 733) on display 1";
        assert_eq!(scrubber().scrub(line), line);
    }

    #[test]
    fn scrubbing_without_a_configured_output_dir_still_removes_the_home() {
        let s = Scrubber::new(Some(PathBuf::from("/Users/anna")), None);
        assert_eq!(s.scrub("/Users/anna/x.log"), "~/x.log");
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
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempdir().unwrap();
        set_active_reports_dir(dir.path().to_path_buf());
        set_mode(ReportMode::Ask);
        set_phase(Phase::Recording);

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
        let reports: Vec<_> = list_reports(dir.path())
            .into_iter()
            .filter(|r| r.message.contains("Bildschirmfoto"))
            .collect();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(report.kind, ReportKind::Panic);
        assert_eq!(report.phase, Phase::Recording);
        assert_eq!(report.consent, Consent::Pending);
        assert!(report.message.contains("Bildschirmfoto"));
        let location = report.location.as_deref().expect("panic location");
        assert!(location.contains("error_reports.rs"), "location was {location}");
    }

    #[test]
    fn the_hook_is_not_installed_when_reports_are_off() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
            !list_reports(dir.path())
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
        write_report(dir.path(), &report).unwrap();

        assert!(decide(dir.path(), &report.report_id, false, None).is_none());
        assert!(list_reports(dir.path()).is_empty());
    }

    #[test]
    fn a_grant_records_the_comment_and_keeps_the_file() {
        let dir = tempdir().unwrap();
        let mut report = sample();
        report.comment = None;
        write_report(dir.path(), &report).unwrap();

        let granted = decide(
            dir.path(),
            &report.report_id,
            true,
            Some("  Ich wollte die Aufnahme stoppen  ".to_string()),
        )
        .unwrap();
        assert_eq!(granted.consent, Consent::Granted);
        assert_eq!(granted.comment.as_deref(), Some("Ich wollte die Aufnahme stoppen"));

        let on_disk = &list_reports(dir.path())[0];
        assert_eq!(on_disk.consent, Consent::Granted);
        assert_eq!(on_disk.comment, granted.comment);
    }

    #[test]
    fn a_grant_without_a_comment_carries_none() {
        let dir = tempdir().unwrap();
        let mut report = sample();
        report.comment = None;
        write_report(dir.path(), &report).unwrap();

        let granted = decide(dir.path(), &report.report_id, true, Some("   ".to_string())).unwrap();
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
        write_report(dir.path(), &stale).unwrap();
        write_report(dir.path(), &fresh).unwrap();

        assert_eq!(sweep_stale(dir.path(), now), 1);
        let remaining = list_reports(dir.path());
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].report_id, fresh.report_id);
    }

    #[test]
    fn nothing_is_written_when_reports_are_off() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempdir().unwrap();
        set_active_reports_dir(dir.path().to_path_buf());
        set_mode(ReportMode::Never);

        assert!(create(ReportKind::CommandError, Phase::Idle, "Upload failed", None).is_none());
        assert!(list_reports(dir.path()).is_empty());
        set_mode(ReportMode::Ask);
    }

    #[test]
    fn a_created_report_carries_the_scrubbed_ring_and_the_job_id() {
        let _guard = RING_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        assert_eq!(list_reports(dir.path()).len(), 1);
    }
}
