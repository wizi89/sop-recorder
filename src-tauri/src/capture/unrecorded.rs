//! Unrecorded windows (#65): windows whose app runs with more rights than the
//! recorder, so their clicks and Return presses never reach its input hooks.
//! The recorder cannot capture them; it notes when one was in front, so the
//! recording bar can say so and the review screen can show the limbo.
//!
//! The visits are kept in `unrecorded.json` in the recording folder. The
//! upload never reads it: nothing about limbos leaves the machine.

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod watch;
#[cfg(windows)]
pub use watch::Watch;

/// Off Windows there are no unrecorded windows, and nothing constructs one.
#[cfg(not(windows))]
pub struct Watch;

pub const FILE: &str = "unrecorded.json";

/// The integrity level UIPI compares: the RID of a token's mandatory label.
pub const MEDIUM_LEVEL: u32 = 0x2000;
/// The level of the Windows security prompt's secure desktop.
pub const SYSTEM_LEVEL: u32 = 0x4000;

/// What was in front. Window titles are never kept: they can hold private
/// content.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "name")]
pub enum UnrecordedWindow {
    App(String),
    /// The app's name could not be read.
    UnknownApp,
    /// The Windows security prompt (UAC) on the secure desktop.
    SecurityPrompt,
}

/// One time an unrecorded window was in front, in seconds since
/// the recording started (the clock of the steps' `timestamp_seconds`).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Visit {
    pub window: UnrecordedWindow,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// Whether a window whose process has `level` is unrecorded for a recorder
/// at `recorder`. A level that cannot be read counts as unrecorded: a needless
/// hint costs little, a silent loss is the bug.
pub fn is_unrecorded(level: Option<u32>, recorder: u32) -> bool {
    level.is_none_or(|level| level > recorder)
}

/// The window a foreground event decides from: the one actually in front,
/// the event's own only when nothing is in front. Out-of-context events arrive
/// late, so an event can name a window that is no longer in front (#108).
pub fn deciding_window<H>(event: H, front: Option<H>) -> H {
    front.unwrap_or(event)
}

/// One recording's visits, written to the folder each time one ends, so a
/// crash keeps the ones before it.
pub struct Log {
    path: PathBuf,
    open: Option<(UnrecordedWindow, f64)>,
    visits: Vec<Visit>,
}

impl Log {
    pub fn new(output_dir: &Path) -> Log {
        Log {
            path: output_dir.join(FILE),
            open: None,
            visits: Vec::new(),
        }
    }

    /// The foreground changed at `now`: to an unrecorded window, or (`None`)
    /// to a recorded one. Returns whether an unrecorded window is now in front.
    pub fn foreground(&mut self, now: f64, window: Option<UnrecordedWindow>) -> bool {
        if let (Some((open, _)), Some(next)) = (&self.open, &window) {
            if open == next {
                return true;
            }
        }
        if let Some((open, start)) = self.open.take() {
            self.visits.push(Visit {
                window: open,
                start_seconds: start,
                end_seconds: now,
            });
            self.write();
        }
        self.open = window.map(|w| (w, now));
        self.open.is_some()
    }

    fn write(&self) {
        let written = serde_json::to_string_pretty(&self.visits)
            .map_err(std::io::Error::from)
            .and_then(|json| fs::write(&self.path, json));
        if let Err(e) = written {
            log::warn!(
                "Unrecorded windows: {} not written: {e}",
                self.path.display()
            );
        }
    }
}

/// The visits of a recording; none when it had no unrecorded window.
pub fn read(output_dir: &Path) -> Result<Vec<Visit>, String> {
    match fs::read_to_string(output_dir.join(FILE)) {
        Ok(json) => serde_json::from_str(&json).map_err(|e| format!("{FILE}: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{FILE}: {e}")),
    }
}

#[cfg(test)]
mod tests;
