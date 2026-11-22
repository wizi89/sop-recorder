use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    MouseClick {
        pos: Option<(i32, i32)>,
    },
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
pub fn should_suppress(
    event: &CaptureEvent,
    mouse_baseline: Option<((i32, i32), Duration)>,
) -> bool {
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

/// One recording's listening state, or `None` between recordings.
///
/// This slot exists because `rdev::listen` cannot be called twice. It parks its
/// callback in a process-global that the next call overwrites, installs a
/// `WH_MOUSE_LL` hook that nothing ever removes, and then blocks in
/// `GetMessageA` forever. Calling it per recording therefore accumulated one OS
/// hook per recording while all of them dispatched to the newest closure, so the
/// N-th recording of an app session saw every physical click N times: one
/// capture followed by N-1 same-position events 0-1 ms later. The old flat
/// debounce swallowed those silently, which is why this survived until every
/// suppression started being logged.
///
/// So the hook is installed exactly once for the process, and starting a
/// recording swaps the session through here instead.
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Whether the process-wide hook is installed, or is being installed right now.
///
/// A `Once` would be the obvious fit and was the wrong one on macOS: there
/// `listen` is a `CGEventTapCreate` that fails immediately, rather than
/// blocking forever, when Accessibility has not been granted. A `Once` counts
/// that failure as the installation, so a user who grants the permission and
/// starts a second recording gets no hook and no second attempt for the life of
/// the process. This is cleared when `listen` returns an error, so the next
/// recording tries again.
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// The recorder's own window, as (x, y, width, height) in the coordinate space
/// `get_cursor_position` reports clicks in: logical points, origin at the
/// top-left of the primary display.
///
/// Windows identifies the recorder's window by HWND and asks the OS what lies
/// under the cursor. macOS cannot answer that question from here -- the only
/// APIs that do are AppKit, which traps the process when touched off the main
/// thread, and the tap callback this serves runs on its own thread. So the
/// window reports where it is whenever it moves, and the check is arithmetic.
///
/// `None` between recordings, and whenever the window's position is unknown;
/// both mean "suppress nothing", which is the safe direction. A surplus step
/// costs attention, a missing one costs the recording.
static EXCLUDED_REGION: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

/// Tell the input hook where the recorder's window is, or that it is gone.
pub fn set_excluded_region(region: Option<(i32, i32, i32, i32)>) {
    *EXCLUDED_REGION.lock().unwrap_or_else(|e| e.into_inner()) = region;
}

/// Whether a point falls inside the recorder's own window.
///
/// Split out from the platform hook so it can be tested without a window, a
/// cursor, or a screen.
///
/// Its only non-test caller is the macOS hook, so on every other platform it
/// is exercised by the tests alone.
#[cfg_attr(
    not(target_os = "macos"),
    allow(dead_code, reason = "only the macOS hook calls it outside tests")
)]
fn point_is_in_region(point: (i32, i32), region: Option<(i32, i32, i32, i32)>) -> bool {
    let Some((x, y, w, h)) = region else {
        return false;
    };
    let (px, py) = point;
    px >= x && px < x + w && py >= y && py < y + h
}

struct Session {
    exclude_hwnd: Option<isize>,
    started_at: Instant,
    baseline: Baseline,
    on_event: Arc<dyn Fn(CaptureEvent) + Send + Sync>,
}

pub struct InputHookHandle {
    _private: (),
}

impl InputHookHandle {
    /// End the recording's session. The OS hook stays installed for the life of
    /// the process, because rdev offers no way to remove it, but with no session
    /// it dispatches nothing.
    pub fn stop(&self) {
        end_session();
    }
}

fn begin_session(exclude_hwnd: Option<isize>, on_event: Arc<dyn Fn(CaptureEvent) + Send + Sync>) {
    *SESSION.lock().unwrap_or_else(|e| e.into_inner()) = Some(Session {
        exclude_hwnd,
        started_at: Instant::now(),
        baseline: Baseline::default(),
        on_event,
    });
}

fn end_session() {
    *SESSION.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // The bar goes back to being an ordinary window between recordings.
    set_excluded_region(None);
}

/// Start listening for global mouse clicks and Enter key. Calls `on_event`
/// directly in the listener thread for each captured event. Screenshots are
/// taken immediately -- no queuing.
pub fn start_listener_with_callback<F>(exclude_hwnd: Option<isize>, on_event: F) -> InputHookHandle
where
    F: Fn(CaptureEvent) + Send + Sync + 'static,
{
    begin_session(exclude_hwnd, Arc::new(on_event));

    if HOOK_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        thread::spawn(|| {
            // Both backends own this thread for the life of the process on
            // success and return only on failure.
            if let Err(e) = install_os_hook() {
                log::error!(
                    "Input hook listener error: {}. No input will be captured for this recording. \
                     On macOS this is what an ungranted Accessibility permission looks like.",
                    e,
                );
                HOOK_INSTALLED.store(false, Ordering::SeqCst);
            }
        });
    }

    InputHookHandle { _private: () }
}

/// The three OS input events this module reacts to, named independently of
/// whichever backend reported them.
///
/// The backends do not agree on a representation and cannot: on macOS the one
/// this used to share with Windows is the thing that crashes (see
/// `macos_hook`), so the shared decision logic below is reached through this
/// instead of through any one platform's event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawEvent {
    LeftMouseDown,
    ReturnPress,
    ReturnRelease,
}

/// The single OS-level callback. Does nothing unless a recording is in session.
fn dispatch(event: RawEvent) {
    let mut guard = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    let Some(session) = guard.as_mut() else {
        return;
    };

    let now = Instant::now();

    // Ignore clicks during the startup guard (the "Start" button click). Timed
    // from the session, not from the process, so it still guards the second
    // recording of a session as well as the first.
    if now.duration_since(session.started_at) < Duration::from_millis(STARTUP_GUARD_MS) {
        return;
    }

    let captured = match event {
        RawEvent::LeftMouseDown => {
            // Ignore clicks on the recorder window (works even if window is moved).
            // Logged like every other drop: D4 asks that no path discards an
            // event without leaving a trace, and this one is easy to overlook
            // because it reads as routing rather than as suppression.
            if is_click_on_excluded_window(session.exclude_hwnd) {
                let (x, y) = get_cursor_position().unwrap_or_default();
                log::info!(
                    "input_hooks: ignored click at ({}, {}), landed on the recorder window",
                    x,
                    y,
                );
                return;
            }

            // Sampled here rather than carried by the event, so it is read the
            // same way on both platforms. This is the same position the overlay
            // is later drawn at, so the suppression decision and the picture
            // agree.
            let pos = get_cursor_position();
            let captured = CaptureEvent::MouseClick { pos };
            let since = session
                .baseline
                .mouse
                .map(|(p, at)| (p, now.duration_since(at)));

            if should_suppress(&captured, since) {
                // Every suppression leaves a trace. A rule that drops events
                // invisibly is the defect being fixed here, and a narrower
                // silent rule keeps the disease.
                let (x, y) = pos.unwrap_or_default();
                log::info!(
                    "input_hooks: suppressed click at ({}, {}), {} ms after the previous capture at the same position",
                    x, y,
                    since.map(|(_, d)| d.as_millis()).unwrap_or(0),
                );
                return;
            }

            // An unknown position cannot serve as a baseline, so it clears one
            // rather than storing a fake origin.
            session.baseline.mouse = pos.map(|p| (p, now));
            captured
        }
        RawEvent::ReturnRelease => {
            note_key_release(&mut session.baseline.keypress);
            return;
        }
        RawEvent::ReturnPress => {
            let since = session.baseline.keypress.since(now);

            if gate_keypress(&mut session.baseline.keypress, now) {
                log::info!(
                    "input_hooks: suppressed Enter auto-repeat, {} ms after the previous keypress",
                    since.map(|d| d.as_millis()).unwrap_or(0),
                );
                return;
            }

            // `gate_keypress` already advanced the key state. It deliberately
            // does NOT touch `baseline.mouse`: a keypress has no position and
            // must not move the baseline the next click is compared against.
            CaptureEvent::EnterKey
        }
    };

    // Released before the callback runs, as the per-recording baseline lock was:
    // `on_event` spawns the capture, and holding the session across it would
    // block every later input event on whatever that spawn does first.
    let on_event = session.on_event.clone();
    drop(guard);
    on_event(captured);
}

mod os;
pub use os::get_cursor_position;
use os::{install_os_hook, is_click_on_excluded_window};

#[cfg(test)]
mod tests;
