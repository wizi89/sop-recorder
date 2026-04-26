use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};

use crate::capture::audio::AudioHandle;
use crate::capture::input_hooks::InputHookHandle;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecordingStatus {
    Idle,
    Recording,
    Processing,
    Done,
    Error(String),
}

impl Default for RecordingStatus {
    fn default() -> Self {
        Self::Idle
    }
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
