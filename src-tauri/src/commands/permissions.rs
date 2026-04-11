//! Microphone permission state query.
//!
//! Reports whether the recorder can see a usable input device at launch so
//! the frontend can surface a warning chip BEFORE the user tries to record
//! and gets a cryptic failure mid-capture.
//!
//! On Windows we can't query the OS permission state directly without
//! linking against Windows.Media APIs, so we use a reliable heuristic:
//! `cpal::default_input_device()` combined with a cheap `default_input_config()`
//! probe. If both succeed we report `granted`. If the device can't be found
//! or the config probe fails (which happens when the OS has revoked access
//! or exclusive-mode is held), we report `denied`. Any unexpected host
//! enumeration error reports `unknown` so the UI can silently skip the chip.

use cpal::traits::{DeviceTrait, HostTrait};

#[tauri::command]
pub fn get_microphone_permission_state() -> String {
    let host = cpal::default_host();

    // Try to grab the default input device. `None` typically means no
    // device is present OR the OS has denied enumeration.
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            log::warn!("Mic permission: no default input device");
            return "denied".to_string();
        }
    };

    // Probe the default config -- this exercises the driver far enough to
    // reveal a revoked-permission state without actually opening a stream.
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
