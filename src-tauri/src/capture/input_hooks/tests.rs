use super::*;

fn click(x: i32, y: i32) -> CaptureEvent {
    CaptureEvent::MouseClick { pos: Some((x, y)) }
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// `SESSION` and `EXCLUDED_REGION` are process-global, correctly so: there
/// is one input hook per process. Cargo runs tests in parallel threads of
/// that one process, so the three tests below were mutating the same
/// session concurrently -- one test's `end_session` landing between
/// another's `begin_session` and its assertion, which failed as
/// "a session must be active" roughly once in twenty-five full runs.
/// Serialising them is the fix; making the state per-thread would be
/// testing something the recorder does not do.
static GLOBAL_SESSION_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Takes the lock without caring whether a previous test poisoned it: a
/// failed assertion in one test must report as that test failing, not as
/// every later test panicking on the lock.
fn global_session_guard() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL_SESSION_TESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// The leak this guards: `rdev::listen` was called once per recording, so
/// the N-th recording of an app session had N OS hooks installed and saw
/// every physical click N times. There is exactly one session, whichever
/// recording last claimed it, and ending it leaves none.
///
/// Exercises `begin_session`/`end_session` rather than
/// `start_listener_with_callback` on purpose: the latter installs a real
/// system-wide input hook, which a test run has no business doing.
#[test]
fn a_second_recording_replaces_the_session_rather_than_adding_one() {
    let _guard = global_session_guard();
    begin_session(None, Arc::new(|_| {}));
    begin_session(Some(42), Arc::new(|_| {}));

    {
        let guard = SESSION.lock().unwrap_or_else(|e| e.into_inner());
        let session = guard.as_ref().expect("a session must be active");
        assert_eq!(
            session.exclude_hwnd,
            Some(42),
            "the newest recording owns the session",
        );
    }

    end_session();
    assert!(
        SESSION.lock().unwrap_or_else(|e| e.into_inner()).is_none(),
        "one stop must end the one session, not leave an earlier one listening",
    );
}

#[test]
fn a_new_session_does_not_inherit_the_previous_baseline() {
    let _guard = global_session_guard();
    begin_session(None, Arc::new(|_| {}));
    SESSION
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .unwrap()
        .baseline
        .mouse = Some(((400, 300), Instant::now()));

    begin_session(None, Arc::new(|_| {}));
    assert!(
        SESSION
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .baseline
            .mouse
            .is_none(),
        "a click at the previous recording's last position must not be suppressed",
    );
    end_session();
}

/// The bug: on macOS every click on the recorder's own bar became a step.
/// Stopping the recording therefore appended a bogus final step, and while
/// the bar needed two clicks to activate, it appended two.
#[test]
fn a_click_on_the_recorder_bar_is_inside_the_region() {
    // The bar as anchored bottom-right: 240x34 at (1218, 840).
    let bar = Some((1218, 840, 240, 34));
    assert!(point_is_in_region((1408, 843), bar), "the Stop button");
    assert!(
        point_is_in_region((1218, 840), bar),
        "top-left corner is in"
    );
}

#[test]
fn a_click_outside_the_recorder_bar_is_a_real_step() {
    let bar = Some((1218, 840, 240, 34));
    assert!(
        !point_is_in_region((1217, 843), bar),
        "one pixel left of it"
    );
    assert!(!point_is_in_region((1408, 839), bar), "one pixel above it");
    assert!(
        !point_is_in_region((829, 318), bar),
        "the far side of the screen"
    );
}

/// The far edges are exclusive: a window at x..x+w does not own the pixel
/// at x+w, which belongs to whatever is next to it.
#[test]
fn the_far_edges_of_the_region_are_exclusive() {
    let bar = Some((1218, 840, 240, 34));
    assert!(!point_is_in_region((1218 + 240, 843), bar));
    assert!(!point_is_in_region((1408, 840 + 34), bar));
}

/// No known region must never suppress: a missing step costs the recording,
/// a surplus one costs attention.
#[test]
fn an_unknown_region_suppresses_nothing() {
    assert!(!point_is_in_region((1408, 843), None));
}

#[test]
fn ending_a_session_forgets_the_region() {
    let _guard = global_session_guard();
    set_excluded_region(Some((1218, 840, 240, 34)));
    end_session();
    assert!(
        !point_is_in_region(
            (1408, 843),
            *EXCLUDED_REGION.lock().unwrap_or_else(|e| e.into_inner())
        ),
        "a stale region would suppress real clicks in the next recording",
    );
}

#[test]
fn same_position_inside_the_window_is_suppressed() {
    assert!(should_suppress(
        &click(400, 300),
        Some(((400, 300), ms(50)))
    ));
}

#[test]
fn same_position_outside_the_window_is_kept() {
    assert!(!should_suppress(
        &click(400, 300),
        Some(((400, 300), ms(SUPPRESSION_WINDOW_MS)))
    ));
    assert!(!should_suppress(
        &click(400, 300),
        Some(((400, 300), ms(2_000)))
    ));
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
