//! The panic hook (D5): a writer and nothing else.

use super::{
    active_reports_dir, collect, mode, os_version, phase, write_report, ReportKind, ReportMode,
};

/// The lock-free record of what the panic hook managed to do. Not a report:
/// it carries no user content, only stage names and timestamps.
pub const TRAIL_FILE: &str = "panic-trail.log";
const TRAIL_MAX_BYTES: u64 = 64 * 1024;

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
pub(super) fn breadcrumb(stage: &str) {
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
    let line = format!("{} {}\n", chrono::Utc::now().to_rfc3339(), stage,);
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
    let report = collect::report(ReportKind::Panic, phase(), &payload, location);

    breadcrumb("collected, about to write");
    // `write_report` announces it; see the note there on why that is not done
    // separately any more.
    match write_report(&dir, &report) {
        Ok(_) => breadcrumb(&format!("wrote report {}", report.report_id)),
        Err(e) => breadcrumb(&format!("stopped: write failed: {}", e)),
    }
}
