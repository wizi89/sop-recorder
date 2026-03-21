use rdev::{listen, Event, EventType};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
/// Returns a handle (to stop) and a receiver for capture events.
pub fn start_listener(
    exclude_rect: Option<(i32, i32, i32, i32)>,
) -> (InputHookHandle, mpsc::Receiver<CaptureEvent>) {
    let (tx, rx) = mpsc::channel();
    let stop_flag = Arc::new(Mutex::new(false));
    let stop_clone = stop_flag.clone();

    thread::spawn(move || {
        let last_event = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
        let tx = tx;
        let stop = stop_clone;

        let callback = move |event: Event| {
            if *stop.lock().unwrap() {
                return;
            }

            let now = Instant::now();
            let mut last = last_event.lock().unwrap();
            if now.duration_since(*last) < Duration::from_millis(DEBOUNCE_MS) {
                return;
            }

            match event.event_type {
                EventType::ButtonPress(rdev::Button::Left) => {
                    // Check if click is within the excluded window rect
                    let _ = exclude_rect; // TODO: self-click exclusion via window position

                    *last = now;
                    // rdev ButtonPress doesn't carry coordinates reliably on Windows
                    // We get position from the OS in the screenshot module
                    let _ = tx.send(CaptureEvent::MouseClick { x: 0.0, y: 0.0 });
                }
                EventType::KeyPress(rdev::Key::Return) => {
                    *last = now;
                    let _ = tx.send(CaptureEvent::EnterKey);
                }
                _ => {}
            }
        };

        if let Err(e) = listen(callback) {
            log::error!("Input hook listener error: {:?}", e);
        }
    });

    (InputHookHandle { stop_flag }, rx)
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
