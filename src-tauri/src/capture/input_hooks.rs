use rdev::{listen, Event, EventType};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

const DEBOUNCE_MS: u64 = 300;

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    MouseClick { x: f64, y: f64 },
    EnterKey,
}

pub struct InputHookHandle {
    stop_flag: Arc<Mutex<bool>>,
}

impl InputHookHandle {
    pub fn stop(&self) {
        *self.stop_flag.lock().unwrap() = true;
    }
}

/// Start listening for global mouse clicks and Enter key in a background thread.
/// Calls `on_event` directly in the listener thread for each captured event.
/// Screenshots are taken immediately -- no queuing.
pub fn start_listener_with_callback<F>(
    exclude_hwnd: Option<isize>,
    on_event: F,
) -> InputHookHandle
where
    F: Fn(CaptureEvent) + Send + 'static,
{
    let stop_flag = Arc::new(Mutex::new(false));
    let stop_clone = stop_flag.clone();

    thread::spawn(move || {
        let started_at = Instant::now();
        let last_event = Arc::new(Mutex::new(Instant::now()));
        let stop = stop_clone;

        let callback = move |event: Event| {
            if *stop.lock().unwrap() {
                return;
            }

            let now = Instant::now();

            // Ignore clicks during the first 500ms (the "Start" button click)
            if now.duration_since(started_at) < Duration::from_millis(500) {
                return;
            }

            let mut last = last_event.lock().unwrap();
            if now.duration_since(*last) < Duration::from_millis(DEBOUNCE_MS) {
                return;
            }

            match event.event_type {
                EventType::ButtonPress(rdev::Button::Left) => {
                    // Ignore clicks on the recorder window (works even if window is moved)
                    if is_click_on_excluded_window(exclude_hwnd) {
                        return;
                    }
                    *last = now;
                    on_event(CaptureEvent::MouseClick { x: 0.0, y: 0.0 });
                }
                EventType::KeyPress(rdev::Key::Return) => {
                    *last = now;
                    on_event(CaptureEvent::EnterKey);
                }
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            log::error!("Input hook listener error: {:?}", e);
        }
    });

    InputHookHandle { stop_flag }
}

/// Check if the click landed on the excluded window (by HWND).
/// Uses WindowFromPoint to get the window under the cursor at click time,
/// then walks up the parent chain to see if it belongs to the excluded window.
#[cfg(windows)]
fn is_click_on_excluded_window(exclude_hwnd: Option<isize>) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, WindowFromPoint, GetAncestor, GA_ROOT};
    use windows::Win32::Foundation::POINT;

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

#[cfg(not(windows))]
fn is_click_on_excluded_window(_exclude_hwnd: Option<isize>) -> bool {
    false
}

/// Get the current mouse cursor position (Windows).
#[cfg(windows)]
pub fn get_cursor_position() -> Option<(i32, i32)> {
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    use windows::Win32::Foundation::POINT;

    let mut point = POINT::default();
    unsafe {
        if GetCursorPos(&mut point).is_ok() {
            Some((point.x, point.y))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
pub fn get_cursor_position() -> Option<(i32, i32)> {
    None
}
