//! Mode, phase, and the ambient facts a report is built from. Everything here
//! is read by the panic hook, so none of it may block (D5).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use super::{reports_dir, Phase, ReportSettings};

/// The three values of the `error_reports` setting (D1). `#[repr(u8)]` for the
/// same reason as `Phase`: the discriminant is what the atomic holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReportMode {
    Ask,
    Always,
    /// Stays last: the test checks `ALL` has one entry per discriminant, in order.
    Never,
}

impl ReportMode {
    const ALL: [ReportMode; 3] = [ReportMode::Ask, ReportMode::Always, ReportMode::Never];

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
    ReportMode::ALL
        .into_iter()
        .find(|mode| Some(mode.as_str()) == setting)
        .unwrap_or(ReportMode::Ask)
}

static MODE: AtomicU8 = AtomicU8::new(ReportMode::Ask as u8);

/// Atomics rather than a lock, throughout this section: every value here is
/// read by the panic hook, which runs on the panicking thread and may be
/// holding, or racing, any lock in the process (D5).
pub fn set_mode(mode: ReportMode) {
    MODE.store(mode as u8, Ordering::Relaxed);
}

pub fn mode() -> ReportMode {
    let stored = MODE.load(Ordering::Relaxed) as usize;
    ReportMode::ALL
        .get(stored)
        .copied()
        .unwrap_or(ReportMode::Ask)
}

static PHASE: AtomicU8 = AtomicU8::new(Phase::Startup as u8);

/// What the app is doing now, for a report the UI cannot annotate itself --
/// which means a panic. Reports the webview creates carry the phase the
/// webview knows, which is finer.
pub fn set_phase(phase: Phase) {
    PHASE.store(phase as u8, Ordering::Relaxed);
}

pub fn phase() -> Phase {
    Phase::from_u8(PHASE.load(Ordering::Relaxed))
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
pub(super) static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

#[cfg(test)]
mod tests;
