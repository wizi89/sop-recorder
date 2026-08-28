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
//!
//! Accessibility (macOS only): the input hook is a `CGEventTapCreate` at the
//! HID tap, which macOS refuses outright without Accessibility. The refusal is
//! a null tap on a background thread, so an ungranted recorder looks like it is
//! recording and simply produces no steps at all -- the worst failure the app
//! has, because the user only finds out once the recording is over. Probed
//! with `AXIsProcessTrusted`, prompted with `AXIsProcessTrustedWithOptions`.
//! Windows reports `granted`: its `WH_MOUSE_LL` hook needs no such grant.

use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
}

/// Read the Accessibility grant, optionally asking macOS to show the prompt.
///
/// The prompting form is the only one that opens the dialog, and it opens it at
/// most once per app per install: after the user has answered, macOS silently
/// returns the stored answer. Both forms return the state as it stands *now*,
/// which is `false` immediately after prompting -- granting Accessibility
/// happens in System Settings, not in the dialog.
#[cfg(target_os = "macos")]
fn accessibility_trusted(prompt: bool) -> bool {
    if !prompt {
        return unsafe { AXIsProcessTrusted() };
    }

    extern "C" {
        fn CFDictionaryCreate(
            allocator: *const std::ffi::c_void,
            keys: *const *const std::ffi::c_void,
            values: *const *const std::ffi::c_void,
            num_values: isize,
            key_callbacks: *const std::ffi::c_void,
            value_callbacks: *const std::ffi::c_void,
        ) -> *const std::ffi::c_void;
        fn CFStringCreateWithCString(
            alloc: *const std::ffi::c_void,
            c_str: *const std::ffi::c_char,
            encoding: u32,
        ) -> *const std::ffi::c_void;
        fn CFRelease(cf: *const std::ffi::c_void);
        static kCFBooleanTrue: *const std::ffi::c_void;
        static kCFTypeDictionaryKeyCallBacks: std::ffi::c_void;
        static kCFTypeDictionaryValueCallBacks: std::ffi::c_void;
    }

    // The key is documented as the constant `kAXTrustedCheckOptionPrompt`, but
    // it lives in a framework this crate does not link, so it is rebuilt from
    // the string it is defined as. 0x0800_0100 is kCFStringEncodingUTF8.
    let key_name = c"AXTrustedCheckOptionPrompt";
    unsafe {
        let key = CFStringCreateWithCString(std::ptr::null(), key_name.as_ptr(), 0x0800_0100);
        if key.is_null() {
            return AXIsProcessTrusted();
        }
        let keys = [key];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        let trusted = if options.is_null() {
            AXIsProcessTrusted()
        } else {
            let trusted = AXIsProcessTrustedWithOptions(options);
            CFRelease(options);
            trusted
        };
        CFRelease(key);
        trusted
    }
}

/// Whether the global input hook can be installed.
///
/// Without this the recorder starts, shows a running timer, and captures
/// nothing: `rdev::listen` gets a null event tap and fails on its own thread.
#[tauri::command]
pub fn get_accessibility_permission_state() -> String {
    #[cfg(target_os = "macos")]
    {
        let granted = accessibility_trusted(false);
        log::info!(
            "Accessibility permission: {}",
            if granted { "granted" } else { "denied" }
        );
        return if granted {
            "granted".to_string()
        } else {
            "denied".to_string()
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        "granted".to_string()
    }
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

        // Accessibility is granted in System Settings rather than in the
        // dialog, so this reads `denied` on the run that first prompts. The
        // banner stays up until the user comes back, which is the honest
        // report: the hook genuinely cannot be installed until they do.
        let accessibility_granted = accessibility_trusted(true);
        let accessibility = if accessibility_granted {
            "granted".to_string()
        } else {
            "denied".to_string()
        };

        log::info!(
            "Permission bootstrap (macOS): mic={}, screen_recording={}, accessibility={}",
            mic, screen, accessibility,
        );
        return PermissionsState { microphone: mic, screen_recording: screen, accessibility };
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionsState {
            microphone: get_microphone_permission_state(),
            screen_recording: "granted".to_string(),
            accessibility: "granted".to_string(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct PermissionsState {
    pub microphone: String,
    pub screen_recording: String,
    pub accessibility: String,
}
