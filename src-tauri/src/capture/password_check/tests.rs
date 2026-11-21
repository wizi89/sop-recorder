use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize};

const W: isize = 0x10;
const FIELD: Rect = Rect::new(300, 300, 500, 330);
const WHOLE: Rect = Rect::new(0, 0, 1920, 1080);
const BUDGET: Duration = Duration::from_millis(100);

fn checker(query: impl FieldQuery + 'static) -> Arc<Checker> {
    Checker::new(Arc::new(query), BUDGET)
}

/// Checked twice, `WARM_UP_GAP` apart, before now.
fn warmed(c: &Checker, hwnd: isize, fields: Vec<Rect>) {
    let now = Instant::now();
    c.seed(
        hwnd,
        fields,
        vec![
            now - Duration::from_millis(400),
            now - Duration::from_millis(200),
        ],
    );
}

fn one_window() -> Vec<(isize, Vec<Rect>)> {
    vec![(W, vec![WHOLE])]
}

#[test]
fn a_checked_window_with_a_field_is_masked_and_not_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![FIELD]));
    warmed(&c, W, vec![FIELD]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(
        out,
        CheckOutcome {
            masks: vec![FIELD],
            flags: vec![]
        }
    );
}

/// An empty window list means the list could not be read, not that the
/// area holds no password field.
#[test]
fn an_empty_window_list_is_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let out = c.check(&[], Instant::now(), false);
    assert_eq!(out.flags.len(), 1, "{:?}", out.flags);
    assert!(out.flags[0].contains("no window was found"));
}

#[test]
fn a_window_never_checked_is_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.flags.len(), 1, "{:?}", out.flags);
    assert!(out.flags[0].contains("not been checked twice"));
}

/// A2: Chromium's first query is complete and empty, so one query is not
/// enough, and neither are two in quick succession.
#[test]
fn a_window_checked_once_or_twice_too_close_is_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let now = Instant::now();
    c.seed(W, vec![], vec![now - Duration::from_millis(300)]);
    assert!(!c.check(&one_window(), now, false).flags.is_empty(), "once");

    c.seed(
        W,
        vec![],
        vec![
            now - Duration::from_millis(300),
            now - Duration::from_millis(250),
        ],
    );
    assert!(
        !c.check(&one_window(), now, false).flags.is_empty(),
        "50 ms apart"
    );
}

/// Queries that complete after the pixels do not make the step checked.
#[test]
fn checks_after_the_pixels_do_not_count() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let pixels_at = Instant::now();
    c.seed(
        W,
        vec![],
        vec![
            pixels_at + Duration::from_millis(1),
            pixels_at + Duration::from_millis(200),
        ],
    );
    assert!(!c.check(&one_window(), pixels_at, false).flags.is_empty());
}

/// Enter in the password field: the field is gone before any query after
/// the event (spike). The cached rectangle still covers what the pixels
/// show, and the user is sent to look.
#[test]
fn a_field_that_vanished_after_submit_is_masked_from_the_cache_and_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![FIELD]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.masks, vec![FIELD]);
    assert!(out
        .flags
        .iter()
        .any(|f| f.contains("changed since the last check")));
}

/// A login form inserted into an existing tab, filled by keyboard: no
/// window event, but focusing the field reported it.
#[test]
fn a_focused_field_is_masked_even_when_the_query_no_longer_finds_it() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![]);
    c.note_focused_field(W, FIELD);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.masks, vec![FIELD]);
    assert!(!out.flags.is_empty());
}

/// A query that completes after this instant, as a warm-up or another
/// step's check would while this step waits for its own.
fn completed(c: &Checker, hwnd: isize, at: Instant, fields: Vec<Rect>) {
    c.cache
        .lock()
        .unwrap()
        .entry(hwnd)
        .or_default()
        .completions
        .push((at, fields));
}

/// Review of #26: Enter in the password field, the pixels are read with the
/// field on screen, and a warm-up finishes after the read with the field
/// gone. The fresh query finds nothing either. The field is masked from
/// what was known at the read, and the change is flagged.
#[test]
fn a_query_finishing_after_the_read_does_not_unmask_the_field() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![FIELD]);
    let pixels_at = Instant::now();
    completed(&c, W, pixels_at + Duration::from_millis(5), vec![]);
    let out = c.check(&one_window(), pixels_at, false);
    assert_eq!(out.masks, vec![FIELD]);
    assert!(
        out.flags
            .iter()
            .any(|f| f.contains("changed since the last check")),
        "{:?}",
        out.flags
    );
}

/// A focused field is kept when a later query misses it (A2: Chromium's
/// first query can be complete and empty).
#[test]
fn a_focused_field_survives_one_query_that_misses_it() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![]);
    c.note_focused_field(W, FIELD);
    completed(&c, W, Instant::now() + Duration::from_millis(1), vec![]);
    std::thread::sleep(Duration::from_millis(5));
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.masks, vec![FIELD]);
}

/// Once the window has been checked twice since the focus, both before the
/// read, and neither found the field, it is gone and no longer masked.
#[test]
fn a_focused_field_is_dropped_after_two_checks_that_miss_it() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![]);
    c.note_focused_field(W, FIELD);
    let after_focus = Instant::now() + Duration::from_millis(1);
    completed(&c, W, after_focus, vec![]);
    completed(&c, W, after_focus + WARM_UP_GAP, vec![]);
    let pixels_at = after_focus + WARM_UP_GAP + Duration::from_millis(1);
    let out = c.check(&one_window(), pixels_at, false);
    assert!(out.masks.is_empty(), "{:?}", out.masks);
}

#[test]
fn a_new_field_found_by_the_fresh_query_is_masked_and_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![FIELD]));
    warmed(&c, W, vec![]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.masks, vec![FIELD]);
    assert!(out.flags.iter().any(|f| f.contains("changed")));
}

#[test]
fn an_empty_field_rectangle_is_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![Rect::new(0, 0, 0, 0)]));
    warmed(&c, W, vec![]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert!(
        out.flags.iter().any(|f| f.contains("still being built")),
        "{:?}",
        out.flags
    );
    assert!(out.masks.is_empty());
}

/// A1: an elevated window answers with nothing, so it is not asked.
#[test]
fn a_window_that_cannot_be_checked_is_flagged() {
    let c = checker(|_| QueryResult::NotCheckable("process runs as administrator"));
    warmed(&c, W, vec![]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert!(out.flags.iter().any(|f| f.contains("administrator")));
}

#[test]
fn a_window_closed_after_the_read_keeps_its_cached_fields_unflagged() {
    let c = checker(|_| QueryResult::Gone);
    warmed(&c, W, vec![FIELD]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.flags, Vec::<String>::new());
    assert_eq!(out.masks, vec![FIELD]);
}

#[test]
fn a_window_closed_before_it_was_ever_checked_is_flagged() {
    let c = checker(|_| QueryResult::Gone);
    let out = c.check(&one_window(), Instant::now(), false);
    assert!(
        out.flags.iter().any(|f| f.contains("not been checked")),
        "{:?}",
        out.flags
    );
}

#[test]
fn a_failed_query_is_flagged_and_cached_fields_are_still_masked() {
    let c = checker(|_| QueryResult::Failed("0x80040201".into()));
    warmed(&c, W, vec![FIELD]);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.masks, vec![FIELD]);
    assert!(out.flags.iter().any(|f| f.contains("0x80040201")));
}

#[test]
fn a_foreground_change_during_the_read_is_flagged() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![]);
    let out = c.check(&one_window(), Instant::now(), true);
    assert_eq!(out.flags.len(), 1);
}

/// Fields of a window are masked only where the window is visible.
#[test]
fn a_field_is_masked_only_in_its_windows_visible_part() {
    let c = checker(|_| QueryResult::Fields(vec![FIELD]));
    warmed(&c, W, vec![FIELD]);
    let visible = vec![(W, vec![Rect::new(0, 0, 400, 1080)])];
    let out = c.check(&visible, Instant::now(), false);
    assert_eq!(out.masks, vec![Rect::new(300, 300, 400, 330)]);
}

/// D6, task 4.2: a hanging app. Two steps each run out of budget on a
/// different hung window and leave their queries running; the third skips
/// the check and is flagged.
#[test]
fn after_two_abandoned_queries_the_next_step_skips_the_check() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let c = checker(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_secs(2));
        QueryResult::Fields(vec![])
    });
    for h in 1..=3 {
        warmed(&c, h, vec![FIELD]);
    }

    for step in 1..=2 {
        let started = Instant::now();
        let out = c.check(&[(step, vec![WHOLE])], Instant::now(), false);
        assert!(
            started.elapsed() < BUDGET * 3,
            "step {step} waited past its budget"
        );
        assert!(
            out.flags.iter().any(|f| f.contains("budget")),
            "step {step}: {:?}",
            out.flags
        );
        assert_eq!(out.masks, vec![FIELD], "cached fields still masked");
    }
    // Past the second query's deadline, so both count as abandoned.
    std::thread::sleep(BUDGET);
    assert_eq!(c.abandoned(), 2);

    let out = c.check(&[(3, vec![WHOLE])], Instant::now(), false);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the third step must not query"
    );
    assert!(out.flags.iter().any(|f| f.contains("still hanging")));
    assert_eq!(out.masks, vec![FIELD]);
}

/// Review of #26: a slow warm-up is not a hung app. Two warm-ups running
/// past the step budget, but within their own deadline, leave the next
/// step's check in place.
#[test]
fn slow_warm_ups_do_not_make_a_step_skip_its_check() {
    let c = checker(|h| {
        if h != W {
            std::thread::sleep(BUDGET * 4);
        }
        QueryResult::Fields(vec![])
    });
    warmed(&c, W, vec![]);
    c.warm_up(1);
    c.warm_up(2);
    std::thread::sleep(BUDGET * 2);
    assert_eq!(c.abandoned(), 0);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(out.flags, Vec::<String>::new());
}

/// Review of #26: a window whose earlier query is stuck is not queried
/// again, so steps over a hung app do not pile up threads in it.
#[test]
fn a_window_with_a_stuck_query_is_not_queried_again() {
    const HUNG: isize = 0x20;
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let c = checker(move |h| {
        if h == HUNG {
            counted.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_secs(2));
        }
        QueryResult::Fields(vec![])
    });
    warmed(&c, W, vec![]);
    warmed(&c, HUNG, vec![FIELD]);
    let windows = vec![(W, vec![WHOLE]), (HUNG, vec![WHOLE])];

    let first = c.check(&windows, Instant::now(), false);
    assert!(
        first.flags.iter().any(|f| f.contains("budget")),
        "{:?}",
        first.flags
    );
    std::thread::sleep(BUDGET);

    let second = c.check(&windows, Instant::now(), false);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the hung window must not be asked again"
    );
    assert!(
        second.flags.iter().any(|f| f.contains("not answering")),
        "{:?}",
        second.flags
    );
    assert!(
        !second.flags.iter().any(|f| f.contains("budget")),
        "the healthy window answered in time: {:?}",
        second.flags
    );
    assert_eq!(second.masks, vec![FIELD], "cached fields still masked");
}

/// 8.1 run: windows are queried side by side, so four windows that would
/// take 160% of the budget one after another fit into it together. A
/// budget of its own, ten times the module's: at 100 ms the thread start
/// on a loaded CI runner ate the slack and the test failed on macOS.
#[test]
fn windows_are_checked_side_by_side_within_one_budget() {
    let budget = Duration::from_secs(1);
    let c = Checker::new(
        Arc::new(move |_| {
            std::thread::sleep(budget * 4 / 10);
            QueryResult::Fields(vec![])
        }),
        budget,
    );
    let windows: Vec<_> = (1..=4).map(|h| (h, vec![WHOLE])).collect();
    for (h, _) in &windows {
        warmed(&c, *h, vec![]);
    }
    let out = c.check(&windows, Instant::now(), false);
    assert_eq!(out.flags, Vec::<String>::new());
}

/// Warm-ups go through the same cap: with two queries hanging, none start.
#[test]
fn warm_ups_respect_the_abandoned_cap() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let c = checker(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_secs(2));
        QueryResult::Fields(vec![])
    });
    c.check(&[(1, vec![WHOLE])], Instant::now(), false);
    c.check(&[(2, vec![WHOLE])], Instant::now(), false);
    std::thread::sleep(BUDGET);
    assert_eq!(c.abandoned(), 2);
    c.warm_up(3);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

/// A warm-up queries twice, `WARM_UP_GAP` apart, which is what makes a
/// step on that window checked.
#[test]
fn a_warm_up_makes_the_window_checked() {
    let c = checker(|_| QueryResult::Fields(vec![FIELD]));
    c.warm_up(W);
    std::thread::sleep(WARM_UP_GAP * 3);
    let out = c.check(&one_window(), Instant::now(), false);
    assert_eq!(
        out,
        CheckOutcome {
            masks: vec![FIELD],
            flags: vec![]
        }
    );
}

#[test]
fn a_window_already_being_warmed_up_is_not_queried_again() {
    // The first query blocks until the test releases it, so a slow runner
    // cannot finish the warm-up before the second call (CI flake on macOS).
    let calls = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicBool::new(false));
    let (counted, gate) = (calls.clone(), released.clone());
    let c = checker(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        while !gate.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        QueryResult::Fields(vec![])
    });
    c.warm_up(W);
    let start = Instant::now();
    while calls.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(1));
    }
    c.warm_up(W);
    std::thread::sleep(Duration::from_millis(30));
    let seen = calls.load(Ordering::SeqCst);
    released.store(true, Ordering::SeqCst);
    assert_eq!(seen, 1);
}
