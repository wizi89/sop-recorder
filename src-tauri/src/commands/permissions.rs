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

// Only the non-macOS microphone check uses cpal now; macOS asks AVFoundation,
// which is the layer that actually knows the answer.
#[cfg(not(target_os = "macos"))]
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

/// The microphone's real TCC state, from the API that actually knows it.
///
/// This used to probe cpal: `default_input_device()` plus
/// `default_input_config()`. Both are CoreAudio *property* queries and need no
/// permission whatsoever, so the probe answered "granted" on any machine with
/// a microphone attached -- including a fresh install that had never been
/// granted anything. The permission screen showed a tick next to a permission
/// nobody had given, and `request_all_permissions` never prompted for the mic
/// because nothing there opened a stream.
///
/// Getting this wrong is not cosmetic. macOS does not fail a denied capture;
/// it vends silent samples. The recording completes, the SOP generates, and
/// the narration is simply empty.
#[cfg(target_os = "macos")]
#[tauri::command]
pub fn get_microphone_permission_state() -> String {
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
        log::warn!("Mic permission: AVMediaTypeAudio unavailable");
        return "denied".to_string();
    };
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };

    // "undetermined" is reported separately from "denied" because only one of
    // them can be fixed by asking. macOS shows the permission dialog when the
    // status is NotDetermined and never again: once the user has refused,
    // `requestAccessForMediaType` is a silent no-op. Collapsing the two left
    // the setup screen offering a button that could not work and would not
    // say so.
    let state = match status {
        AVAuthorizationStatus::Authorized => "granted",
        AVAuthorizationStatus::NotDetermined => "undetermined",
        _ => "denied",
    };
    log::info!("Mic permission: {} (status {})", state, status.0);
    state.to_string()
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub fn get_microphone_permission_state() -> String {
    // Windows has no TCC layer to consult; a usable default input device is
    // the whole of the question there.
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

/// Show the microphone permission dialog, if the user has not decided yet.
///
/// Deliberately not awaited. AVFoundation calls the completion handler on an
/// arbitrary queue whenever the user gets around to answering, and blocking a
/// command on a dialog the user may leave open indefinitely buys nothing --
/// the permission screen already polls.
#[cfg(target_os = "macos")]
fn request_microphone_access() {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};

    let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
        log::warn!("Mic prompt: AVMediaTypeAudio unavailable");
        return;
    };
    let handler = RcBlock::new(|granted: Bool| {
        log::info!("Mic permission dialog answered: granted={}", granted.as_bool());
    });
    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &handler);
    }
}


/// Open the System Settings pane where a refused permission can be restored.
///
/// A command rather than the opener plugin for two reasons. The plugin's
/// default scope covers `http`, `https`, `mailto` and `tel` only, so the
/// `x-apple.systempreferences:` URL was rejected before it ever reached the
/// OS -- the link did nothing at all. And widening that scope would let the
/// frontend hand arbitrary URLs to LaunchServices, when all that is wanted is
/// three fixed destinations. The pane is matched against an allowlist here, so
/// nothing else is reachable through it.
#[tauri::command]
pub fn open_privacy_settings(pane: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let anchor = match pane.as_str() {
            "microphone" => "Privacy_Microphone",
            "screen" => {
                // An app that has never asked for screen capture is not listed
                // in that pane at all, so opening it would show the user a
                // switch that does not exist yet. This call is what puts it in
                // the list; it also raises the dialog while the state is still
                // undetermined, and is a no-op once decided.
                unsafe { CGRequestScreenCaptureAccess() };
                "Privacy_ScreenCapture"
            }
            "accessibility" => "Privacy_Accessibility",
            other => return Err(format!("Unknown privacy pane: {}", other)),
        };
        let url = format!(
            "x-apple.systempreferences:com.apple.preference.security?{}",
            anchor
        );
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Could not open System Settings: {}", e))?;
        log::info!("Opened System Settings at {}", anchor);
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows has no equivalent per-permission pane to send anyone to.
        let _ = pane;
        Err("Privacy panes are a macOS concept".into())
    }
}

#[cfg(test)]
mod privacy_pane_tests {
    use super::open_privacy_settings;

    #[test]
    fn an_unknown_pane_is_refused_rather_than_guessed_at() {
        // The allowlist is the reason this command exists instead of a wider
        // URL scope, so it is the part worth pinning.
        assert!(open_privacy_settings("../../etc".into()).is_err());
        assert!(open_privacy_settings("Privacy_Camera".into()).is_err());
    }
}

/// Raise the prompt for one permission, and report where it stands afterwards.
///
/// Split out from `request_all_permissions` because the setup screen now asks
/// per row. One button that fires all three could only ever be described as
/// "grant everything", which is a promise it cannot keep: macOS shows a dialog
/// only while a permission is undetermined, so with one already refused the
/// button did nothing for it and said nothing about why.
#[tauri::command]
pub fn request_permission(which: String) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        match which.as_str() {
            "microphone" => {
                request_microphone_access();
                Ok(get_microphone_permission_state())
            }
            // Shows the system dialog when undetermined, no-op once decided.
            "screen" => {
                let granted = unsafe { CGRequestScreenCaptureAccess() };
                Ok(if granted { "granted".into() } else { "denied".into() })
            }
            // Granted in System Settings rather than in the dialog, so this
            // reads denied on the run that first prompts. That is the honest
            // report: the hook cannot be installed until the user comes back.
            "accessibility" => {
                let granted = accessibility_trusted(true);
                Ok(if granted { "granted".into() } else { "denied".into() })
            }
            other => Err(format!("Unknown permission: {}", other)),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        match which.as_str() {
            "microphone" => Ok(get_microphone_permission_state()),
            // No privacy layer to prompt against on Windows.
            "screen" | "accessibility" => Ok("granted".to_string()),
            other => Err(format!("Unknown permission: {}", other)),
        }
    }
}

#[cfg(test)]
mod request_permission_tests {
    use super::request_permission;

    #[test]
    fn an_unknown_permission_is_refused_rather_than_guessed_at() {
        assert!(request_permission("camera".into()).is_err());
        assert!(request_permission("".into()).is_err());
    }
}
