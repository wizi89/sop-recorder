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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedStep {
    pub order_id: u32,
    pub filename: String,
    pub click_x: Option<i32>,
    pub click_y: Option<i32>,
}

pub struct RecordingSession {
    pub output_dir: PathBuf,
    pub screenshots_dir: PathBuf,
    pub guide_title: String,
    pub steps: Vec<CapturedStep>,
    pub audio_handle: Option<AudioHandle>,
    pub input_hook: Option<InputHookHandle>,
    pub stop_flag: Option<Arc<AtomicBool>>,
    pub in_flight: Option<Arc<AtomicU32>>,
}

pub struct AppState {
    pub recording_status: Mutex<RecordingStatus>,
    pub current_session: Mutex<Option<RecordingSession>>,
    pub in_flight_captures: Mutex<Option<Arc<AtomicU32>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording_status: Mutex::new(RecordingStatus::default()),
            current_session: Mutex::new(None),
            in_flight_captures: Mutex::new(None),
        }
    }
}
