use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};

use crate::capture::audio::AudioHandle;
use crate::capture::input_hooks::InputHookHandle;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum RecordingStatus {
    #[default]
    Idle,
    Recording,
    Processing,
    Done,
    Error(String),
}

pub struct RecordingSession {
    pub output_dir: PathBuf,
    pub screenshots_dir: PathBuf,
    pub guide_title: String,
    pub audio_handle: Option<AudioHandle>,
    pub input_hook: Option<InputHookHandle>,
    pub stop_flag: Option<Arc<AtomicBool>>,
    pub in_flight: Option<Arc<AtomicU32>>,
    /// Shared screenshot counter so `delete_last_screenshot` can atomically
    /// decrement when the user undoes a captured step.
    pub step_counter: Option<Arc<AtomicU32>>,
    /// Captures that failed for this recording. A failure leaves a gap in the
    /// step numbering, which the upload path now survives -- but the user still
    /// has to be told the recording is short of what they did, rather than
    /// finding out from a guide that is missing a step.
    pub failed_captures: Option<Arc<AtomicU32>>,
}

pub struct AppState {
    pub recording_status: Mutex<RecordingStatus>,
    pub current_session: Mutex<Option<RecordingSession>>,
    pub in_flight_captures: Mutex<Option<Arc<AtomicU32>>>,
    /// Shared stop flag -- lives outside the session mutex so it can be set
    /// immediately in stop_recording without waiting for the session lock.
    pub capture_stop_flag: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording_status: Mutex::new(RecordingStatus::default()),
            current_session: Mutex::new(None),
            in_flight_captures: Mutex::new(None),
            capture_stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    /// Every lock recovers from poisoning. With a plain unwrap, one thread that
    /// panicked while holding a lock -- a capture, the audio callback -- makes
    /// every later command that takes it panic too, and the app is dead until
    /// restarted.
    #[test]
    fn no_lock_panics_on_a_poisoned_mutex() {
        let needle = concat!(".lock()", ".unwrap()");
        let mut offenders = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let source = std::fs::read_to_string(&path).unwrap();
                    for (i, line) in source.lines().enumerate() {
                        if line.contains(needle) {
                            offenders.push(format!("{}:{}", path.display(), i + 1));
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "use .lock().unwrap_or_else(|e| e.into_inner()) instead:\n{}",
            offenders.join("\n")
        );
    }
}
