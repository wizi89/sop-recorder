//! The OS layer of the input hooks: the process-wide hook, the excluded-window
//! test and the cursor position, once per platform.

use super::*;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Install the process-wide OS input hook and run it. Returns only on failure.
#[cfg(windows)]
pub(super) fn install_os_hook() -> Result<(), String> {
    use rdev::{listen, EventType};

    listen(|event| {
        let raw = match event.event_type {
            EventType::ButtonPress(rdev::Button::Left) => RawEvent::LeftMouseDown,
            EventType::KeyPress(rdev::Key::Return) => RawEvent::ReturnPress,
            EventType::KeyRelease(rdev::Key::Return) => RawEvent::ReturnRelease,
            _ => return,
        };
        dispatch(raw);
    })
    .map_err(|e| format!("{:?}", e))
}

/// Install the process-wide OS input hook and run it. Returns only on failure.
///
/// This is a hand-rolled `CGEventTap` rather than `rdev::listen`, which is what
/// Windows uses, because rdev's macOS listener crashes the whole app on the
/// first key pressed during a recording.
///
/// rdev fills in a human-readable `name` for every keyboard event, and does it
/// inside the tap callback via `TISGetInputSourceProperty`. Text Input Services
/// asserts it is called on the main queue; the tap callback runs on this
/// spawned thread's run loop, so the assertion fires and the process dies on
/// `SIGTRAP` in `_dispatch_assert_queue_fail`. There is no way to opt out of
/// that work from outside rdev -- it happens before the event is handed over --
/// and the recorder needs none of it. So the tap is built here, reading only
/// the button and the key code, and never touching TIS.
#[cfg(target_os = "macos")]
pub(super) fn install_os_hook() -> Result<(), String> {
    use core_foundation::base::TCFType;
    use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        EventField,
    };

    /// `kVK_Return` and `kVK_ANSI_KeypadEnter`: the two key codes that mean
    /// "Enter" on a Mac keyboard. Numeric because they are ANSI virtual key
    /// codes, fixed by the hardware layout and independent of the language the
    /// keyboard is set to -- which is the whole reason no layout lookup, and
    /// therefore no TIS call, is needed to recognise them.
    const RETURN_KEY_CODES: [i64; 2] = [36, 76];

    extern "C" {
        fn CGEventTapEnable(tap: core_foundation::mach_port::CFMachPortRef, enable: bool);
    }

    // The tap's own port, so the callback can switch it back on.
    //
    // macOS disables a tap unilaterally if its callback ever runs long, and
    // tells it so by delivering one `TapDisabledByTimeout` event. Nothing else
    // is reported: without this the recorder would carry on showing a running
    // timer and silently capture nothing more for the rest of the recording.
    // The callback runs on this same thread, so a thread-local is all the
    // sharing that is needed.
    thread_local! {
        static TAP_PORT: std::cell::Cell<core_foundation::mach_port::CFMachPortRef> =
            const { std::cell::Cell::new(std::ptr::null_mut()) };
    }

    let tap = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        // Listen only: the recorder observes input, it must never swallow or
        // alter a click the user meant for the app they are recording.
        CGEventTapOptions::ListenOnly,
        vec![
            CGEventType::LeftMouseDown,
            CGEventType::KeyDown,
            CGEventType::KeyUp,
        ],
        |_proxy, event_type, event| {
            let raw = match event_type {
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    log::warn!(
                        "input_hooks: macOS disabled the event tap ({:?}); re-enabling it",
                        event_type,
                    );
                    let port = TAP_PORT.with(|p| p.get());
                    if !port.is_null() {
                        // Safety: the port is this thread's own live tap, and
                        // the run loop it belongs to is the one calling us.
                        unsafe { CGEventTapEnable(port, true) };
                    }
                    None
                }
                CGEventType::LeftMouseDown => Some(RawEvent::LeftMouseDown),
                CGEventType::KeyDown | CGEventType::KeyUp => {
                    let code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                    let down = matches!(event_type, CGEventType::KeyDown);
                    RETURN_KEY_CODES.contains(&code).then_some(if down {
                        RawEvent::ReturnPress
                    } else {
                        RawEvent::ReturnRelease
                    })
                }
                _ => None,
            };
            if let Some(raw) = raw {
                dispatch(raw);
            }
            // ListenOnly: the return value is ignored, and passing the event
            // through unchanged is the only correct thing to express here.
            None
        },
    )
    .map_err(|_| {
        "CGEventTapCreate returned null (Accessibility permission not granted)".to_string()
    })?;

    TAP_PORT.with(|p| p.set(tap.mach_port.as_concrete_TypeRef()));

    let source = tap
        .mach_port
        .create_runloop_source(0)
        .map_err(|_| "failed to create a run loop source for the event tap".to_string())?;

    // Safety: adding a source to this thread's own run loop, with a mode
    // constant owned by CoreFoundation.
    unsafe {
        CFRunLoop::get_current().add_source(&source, kCFRunLoopCommonModes);
    }
    tap.enable();

    // Owns the thread from here on. The tap and its source stay alive with it.
    CFRunLoop::run_current();
    Err("the input hook run loop exited unexpectedly".to_string())
}

/// Check if the click landed on the excluded window (by HWND).
/// Uses WindowFromPoint to get the window under the cursor at click time,
/// then walks up the parent chain to see if it belongs to the excluded window.
#[cfg(windows)]
pub(super) fn is_click_on_excluded_window(exclude_hwnd: Option<isize>) -> bool {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetCursorPos, WindowFromPoint, GA_ROOT,
    };

    let hwnd = match exclude_hwnd {
        Some(h) => HWND(h as *mut _),
        None => return false,
    };

    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point).is_err() {
            return false;
        }
        let hit = WindowFromPoint(point);
        if hit == hwnd {
            return true;
        }
        // The click might be on a child window (webview), so check the root ancestor
        let root = GetAncestor(hit, GA_ROOT);
        root == hwnd
    }
}

/// Whether the click landed on the recorder's own window (macOS).
///
/// Compares the cursor against the region the window last reported. The HWND is
/// meaningless here; the region is the whole input.
#[cfg(target_os = "macos")]
pub(super) fn is_click_on_excluded_window(_exclude_hwnd: Option<isize>) -> bool {
    let Some(cursor) = get_cursor_position() else {
        return false;
    };
    point_is_in_region(
        cursor,
        *EXCLUDED_REGION.lock().unwrap_or_else(|e| e.into_inner()),
    )
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub(super) fn is_click_on_excluded_window(_exclude_hwnd: Option<isize>) -> bool {
    false
}

/// Get the current mouse cursor position (Windows).
#[cfg(windows)]
pub fn get_cursor_position() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT::default();
    unsafe {
        if GetCursorPos(&mut point).is_ok() {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
pub fn get_cursor_position() -> Option<(i32, i32)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let point = event.location();
    Some((point.x.round() as i32, point.y.round() as i32))
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn get_cursor_position() -> Option<(i32, i32)> {
    None
}
