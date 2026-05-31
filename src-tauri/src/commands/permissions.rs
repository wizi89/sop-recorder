//! Permission state queries and bootstrap.
//!
//! Reports whether the recorder has the OS-level permissions it needs so
//! the frontend can surface a warning BEFORE the user tries to record and
//! gets a cryptic failure mid-capture (or worse, on macOS: a silent
//! desktop-wallpaper screenshot when Screen Recording is denied).
//!
//! Microphone (both platforms): heuristic via `cpal::default_input_device()`
//! + `default_input_config()`. Avoids linking Windows.Media; on macOS this
//! also triggers the system permission prompt on first call after a fresh
//! install.
//!
//! Screen Recording (macOS only): `CGPreflightScreenCaptureAccess` reads
//! the TCC state without prompting, `CGRequestScreenCaptureAccess` triggers
//! the prompt. Windows reports `granted` -- there is no analogous TCC layer.

use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[tauri::command]
pub fn get_microphone_permission_state() -> String {
    let host = cpal::default_host();

    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            log::warn!("Mic permission: no default input device");
            return "denied".to_string();
        }
    };

    match device.default_input_config() {
        Ok(_) => {
            log::info!("Mic permission: granted (device {:?})", device.name().ok());
            "granted".to_string()
        }
        Err(e) => {
            log::warn!("Mic permission: config probe failed: {}", e);
            "denied".to_string()
        }
    }
}

#[tauri::command]
pub fn get_screen_recording_permission_state() -> String {
    #[cfg(target_os = "macos")]
    {
        let granted = unsafe { CGPreflightScreenCaptureAccess() };
        log::info!("Screen recording permission: {}", if granted { "granted" } else { "denied" });
        return if granted { "granted".to_string() } else { "denied".to_string() };
    }
    #[cfg(not(target_os = "macos"))]
    {
        "granted".to_string()
    }
}

/// Trigger the macOS TCC prompts for mic + screen recording up-front, so
/// the user grants everything in one sitting instead of being interrupted
/// by a fresh prompt at every recording start. Returns the post-prompt
/// state for both so the UI can immediately re-render without polling.
#[tauri::command]
pub fn request_all_permissions() -> PermissionsState {
    #[cfg(target_os = "macos")]
    {
        // Mic: opening an input stream once is what actually triggers the
        // TCC dialog on macOS. The cpal probe in get_microphone_permission_state
        // is enough -- calling it here serves the dual purpose of priming
        // the prompt and reading back the post-decision state.
        let mic = get_microphone_permission_state();

        // Screen recording: this call shows the system dialog if the state
        // is undetermined and is a no-op if the user has already decided.
        // The return value reflects the *current* (still synchronous) state,
        // which after a fresh decision will be `true` only if the user
        // accepted within the dialog before this returns.
        let screen_granted = unsafe { CGRequestScreenCaptureAccess() };
        let screen = if screen_granted { "granted".to_string() } else { "denied".to_string() };

        log::info!("Permission bootstrap (macOS): mic={}, screen_recording={}", mic, screen);
        return PermissionsState { microphone: mic, screen_recording: screen };
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionsState {
            microphone: get_microphone_permission_state(),
            screen_recording: "granted".to_string(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct PermissionsState {
    pub microphone: String,
    pub screen_recording: String,
}
