//! Building a report: the ring buffer that holds the recent log (D4), and the
//! step that turns a failure into a report ready for the queue.

use std::collections::VecDeque;
use std::sync::Mutex;

use super::{
    active_reports_dir, context, mode, scrubber, write_report, ErrorReport, Phase, ReportKind,
    ReportMode,
};

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
