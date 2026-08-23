use rdev::{listen, Event, EventType};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Ignore input for this long after the listener starts, so the click on the
/// recorder's own "Start" button never becomes step 1.
const STARTUP_GUARD_MS: u64 = 500;

/// A second click at the *same pixel* inside this window is a double-click, not
/// a second deliberate action. Bounded at 300 ms by spec: the rule it replaces
/// dropped everything inside 300 ms, and a wider window here would discard
/// events that rule kept.
const SUPPRESSION_WINDOW_MS: u64 = 250;
const _: () = assert!(SUPPRESSION_WINDOW_MS <= 300);

/// Keypresses have no cursor position, so the position rule cannot apply to
/// them. They are guarded against auto-repeat by the release test instead: the
/// OS delivers a KeyPress stream while Enter is held down, and without a guard
/// every repeat becomes a screenshot.
///
/// A time-only guard cannot do this job, however narrow. Windows waits about
/// 500 ms before the first repeat, which is outside any window bounded at the
/// 300 ms the mouse rule is held to, so the first repeat is indistinguishable
/// from a deliberate second Enter by timing alone.
///
/// The one thing that does distinguish them is a KeyRelease: auto-repeat emits
/// none, a real second press does. The cost is that a release which never
/// arrives, because the key came up while another window had focus or the
/// listener missed it, would latch the key held and suppress every Enter after
/// it. This ceiling bounds that: a repeat stream is dense, roughly 33 ms and
/// never slower than 400 ms even at the slowest OS repeat rate, so a gap this
/// long means the stream stopped and the key is treated as released whether its
/// release was seen or not.
///
/// Known ceiling (design D3): Windows' slowest *initial* repeat delay ("Long"
/// keyboard setting) is also ~1 s, so at that non-default setting the first
/// repeat of a held key reads as a stopped stream and is captured: two steps
/// instead of one. Accepted; widening this would slow lost-release recovery
/// for everyone to fix one surplus step at a rare setting.
const KEY_RELEASE_ASSUMED_MS: u64 = 1_000;

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    /// `pos` is the cursor position at capture time. `None` only when the OS
    /// refused to report it, which is rare and never a reason to drop a step.
    MouseClick { pos: Option<(i32, i32)> },
    EnterKey,
}

/// Decide whether a mouse event is a provable no-op repeat of the previous one.
///
/// `mouse_baseline` is the position of the previously captured mouse event and
/// the time elapsed since it. Suppression requires the same position AND the
/// window: two events at different positions are never suppressed, at any
/// interval. A keypress, an unknown position, or no previous capture all mean
/// "capture" -- a surplus step costs attention, a missing one costs the
/// recording.
pub fn should_suppress(event: &CaptureEvent, mouse_baseline: Option<((i32, i32), Duration)>) -> bool {
    match (event, mouse_baseline) {
        (CaptureEvent::MouseClick { pos: Some(pos) }, Some((baseline, since))) => {
            *pos == baseline && since < Duration::from_millis(SUPPRESSION_WINDOW_MS)
        }
        _ => false,
    }
}

/// Whether the key has been down since its last press, and when that press was.
/// Separate from the mouse baseline on purpose: an Enter pressed straight after
/// a click, mouse unmoved, must not be suppressed, which is the defect this
/// whole rule exists to remove.
#[derive(Default)]
pub struct KeyState {
    last_press: Option<Instant>,
    held: bool,
}

impl KeyState {
    /// Interval since the previous keypress, for the suppression log.
    fn since(&self, now: Instant) -> Option<Duration> {
        self.last_press.map(|at| now.duration_since(at))
    }
}

/// Auto-repeat guard: suppress a keypress only while the key is known to be
/// still down from its previous press. Every keypress advances the state,
/// suppressed ones included, so a held key stays held for the whole stream and
/// yields exactly one capture however long it is held.
///
/// A press arriving after `KEY_RELEASE_ASSUMED_MS` of silence is captured even
/// with no release recorded, so a lost release cannot latch the guard shut.
pub fn gate_keypress(state: &mut KeyState, now: Instant) -> bool {
    let stream_stopped = state
        .last_press
        .is_none_or(|at| now.duration_since(at) >= Duration::from_millis(KEY_RELEASE_ASSUMED_MS));
    let suppressed = state.held && !stream_stopped;

    state.last_press = Some(now);
    state.held = true;
    suppressed
}

/// The key came up, so the next press is a deliberate one, not a repeat.
pub fn note_key_release(state: &mut KeyState) {
    state.held = false;
}

/// The previous capture of each kind. A keypress never writes `mouse`, so it
/// cannot move the baseline the next click is compared against.
#[derive(Default)]
struct Baseline {
    mouse: Option<((i32, i32), Instant)>,
    keypress: KeyState,
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
        let baseline = Arc::new(Mutex::new(Baseline::default()));
        let stop = stop_clone;

        let callback = move |event: Event| {
            if *stop.lock().unwrap() {
                return;
            }

            let now = Instant::now();

            // Ignore clicks during the startup guard (the "Start" button click)
            if now.duration_since(started_at) < Duration::from_millis(STARTUP_GUARD_MS) {
                return;
            }

            match event.event_type {
                EventType::ButtonPress(rdev::Button::Left) => {
                    // Ignore clicks on the recorder window (works even if window is moved)
                    if is_click_on_excluded_window(exclude_hwnd) {
                        return;
                    }

                    // Sampled here, not carried by the event: rdev's ButtonPress
                    // has no coordinates. This is the same position the overlay
                    // is later drawn at, so the suppression decision and the
                    // picture agree.
                    let pos = get_cursor_position();
                    let captured = CaptureEvent::MouseClick { pos };

                    let mut base = baseline.lock().unwrap();
                    let since = base.mouse.map(|(p, at)| (p, now.duration_since(at)));

                    if should_suppress(&captured, since) {
                        // Every suppression leaves a trace. A rule that drops
                        // events invisibly is the defect being fixed here, and a
                        // narrower silent rule keeps the disease.
                        let (x, y) = pos.unwrap_or_default();
                        log::info!(
                            "input_hooks: suppressed click at ({}, {}), {} ms after the previous capture at the same position",
                            x, y,
                            since.map(|(_, d)| d.as_millis()).unwrap_or(0),
                        );
                        return;
                    }

                    // An unknown position cannot serve as a baseline, so it
                    // clears one rather than storing a fake origin.
                    base.mouse = pos.map(|p| (p, now));
                    drop(base);
                    on_event(captured);
                }
                EventType::KeyRelease(rdev::Key::Return) => {
                    note_key_release(&mut baseline.lock().unwrap().keypress);
                }
                EventType::KeyPress(rdev::Key::Return) => {
                    let mut base = baseline.lock().unwrap();
                    let since = base.keypress.since(now);

                    if gate_keypress(&mut base.keypress, now) {
                        log::info!(
                            "input_hooks: suppressed Enter auto-repeat, {} ms after the previous keypress",
                            since.map(|d| d.as_millis()).unwrap_or(0),
                        );
                        return;
                    }

                    // `gate_keypress` already advanced `base.keypress`. It
                    // deliberately does NOT touch `base.mouse`: a keypress has no
                    // position and must not move the baseline the next click is
                    // compared against.
                    drop(base);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn click(x: i32, y: i32) -> CaptureEvent {
        CaptureEvent::MouseClick { pos: Some((x, y)) }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn same_position_inside_the_window_is_suppressed() {
        assert!(should_suppress(&click(400, 300), Some(((400, 300), ms(50)))));
    }

    #[test]
    fn same_position_outside_the_window_is_kept() {
        assert!(!should_suppress(
            &click(400, 300),
            Some(((400, 300), ms(SUPPRESSION_WINDOW_MS)))
        ));
        assert!(!should_suppress(&click(400, 300), Some(((400, 300), ms(2_000)))));
    }

    #[test]
    fn different_position_is_never_suppressed_at_any_interval() {
        for interval in [0, 1, 50, SUPPRESSION_WINDOW_MS - 1, 5_000] {
            assert!(
                !should_suppress(&click(400, 300), Some(((401, 300), ms(interval)))),
                "a click one pixel away was dropped after {} ms",
                interval
            );
        }
    }

    #[test]
    fn first_event_has_no_baseline_and_is_kept() {
        assert!(!should_suppress(&click(400, 300), None));
    }

    #[test]
    fn unknown_position_is_kept() {
        let unknown = CaptureEvent::MouseClick { pos: None };
        assert!(!should_suppress(&unknown, Some(((400, 300), ms(1)))));
    }

    #[test]
    fn keypress_is_never_suppressed_by_the_position_rule() {
        // Enter straight after a click, mouse unmoved. The baseline is a real
        // click position and the interval is inside the window; suppressing here
        // would be the original bug in a new place.
        assert!(!should_suppress(
            &CaptureEvent::EnterKey,
            Some(((400, 300), ms(10)))
        ));
    }

    /// One keyboard event: a press at an offset, or a release.
    enum Key {
        Press(u64),
        Release,
    }
    use Key::{Press, Release};

    /// Feed a sequence of keyboard events through one state and count the
    /// presses that survive. The state is the whole point of the gate, so the
    /// pure predicate this replaced could not have observed what is under test.
    fn captures_from(events: &[Key]) -> usize {
        let start = Instant::now();
        let mut state = KeyState::default();
        events
            .iter()
            .filter(|e| match e {
                Press(off) => !gate_keypress(&mut state, start + ms(*off)),
                Release => {
                    note_key_release(&mut state);
                    false
                }
            })
            .count()
    }

    /// A key held from `press` until `until`: Windows waits about 500 ms before
    /// the first repeat, then delivers them roughly every 33 ms.
    fn held_from(press: u64, until: u64) -> Vec<Key> {
        let mut events = vec![Press(press)];
        let mut t = press + 500;
        while t <= until {
            events.push(Press(t));
            t += 33;
        }
        events
    }

    #[test]
    fn a_held_key_produces_exactly_one_capture() {
        assert_eq!(
            captures_from(&held_from(0, 8_000)),
            1,
            "a held key is still flooding the capture path"
        );
    }

    #[test]
    fn the_first_repeat_is_suppressed_despite_arriving_outside_any_window() {
        // The initial repeat delay (~500 ms) exceeds the 300 ms the mouse rule
        // is bounded by, so no time-only guard can catch this one. Two captures
        // here means the release test is not doing its job.
        assert_eq!(captures_from(&[Press(0), Press(500)]), 1);
    }

    #[test]
    fn a_released_key_reopens_the_gate() {
        let mut events = held_from(0, 2_000);
        events.push(Release);
        events.push(Press(2_500));
        assert_eq!(captures_from(&events), 2);
    }

    #[test]
    fn two_deliberate_presses_inside_the_window_are_both_captured() {
        // The time-only guard this replaced dropped the second. A release
        // between them proves the key came up, so both are real.
        assert_eq!(captures_from(&[Press(0), Release, Press(120)]), 2);
    }

    #[test]
    fn a_lost_release_does_not_latch_the_gate_shut() {
        // The key came up while another window had focus, so no release
        // arrived. The next press must still be captured once the repeat
        // stream has plainly stopped.
        assert_eq!(
            captures_from(&[Press(0), Press(500), Press(533), Press(5_000)]),
            2
        );
    }

    #[test]
    fn the_first_keypress_is_never_suppressed() {
        assert!(!gate_keypress(&mut KeyState::default(), Instant::now()));
    }

    #[test]
    fn the_window_cannot_exceed_the_rule_it_replaces() {
        // The spec bound: a wider window would drop same-position events that
        // the previous time-only rule captured. Also enforced at compile time.
        assert!(SUPPRESSION_WINDOW_MS <= 300);
    }
}
