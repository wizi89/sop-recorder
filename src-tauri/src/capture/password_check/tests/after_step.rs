//! #66: what a step's check leaves behind for the next step, and what a
//! query completing after the step's trigger may not change.

use super::*;

/// Waits until `hwnd` has `n` completions within `LATE_WARM_UP` after
/// `since`, at most 5 s, and returns how many it has: the warm-ups run on
/// threads of their own. The bound keeps the late warm-up of a step from
/// standing in for the warm-up right after it.
fn completions_after(c: &Checker, hwnd: isize, since: Instant, n: usize) -> usize {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let count = c
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&hwnd)
            .map_or(0, |e| {
                e.completions
                    .iter()
                    .filter(|(at, _)| *at > since && *at < since + LATE_WARM_UP)
                    .count()
            });
        if count >= n || Instant::now() > deadline {
            return count;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A page navigated after a step's check (a submitted login, a tab switch).
/// The warm-up after the check reads the new page, so the next step is
/// checked against it: no stale mask, no "changed" flag.
#[test]
fn the_step_after_a_navigation_has_no_stale_mask_and_no_flag() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    // The step's own query still finds the field; every later one finds
    // the new page without it.
    let c = checker(move |_| {
        if counted.fetch_add(1, Ordering::SeqCst) == 0 {
            QueryResult::Fields(vec![FIELD])
        } else {
            QueryResult::Fields(vec![])
        }
    });
    warmed(&c, W, vec![FIELD]);
    let step = Instant::now();
    let first = c.check(&one_window(), step, step, false);
    assert_eq!(first.masks, vec![FIELD]);
    assert_eq!(completions_after(&c, W, step, 3), 3);
    let next = Instant::now();
    assert_eq!(
        c.check(&one_window(), next, next, false),
        CheckOutcome {
            masks: vec![],
            flags: vec![]
        }
    );
}

/// A step's check warms up every window it checked, so the cache holds the
/// step's own completion and the warm-up's two for the next step.
#[test]
fn a_check_leaves_its_own_and_two_warm_up_completions_for_each_window() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let windows: Vec<_> = (1..=2).map(|h| (h, vec![WHOLE])).collect();
    let step = Instant::now();
    c.check(&windows, step, step, false);
    for (h, _) in &windows {
        assert!(completions_after(&c, *h, step, 3) >= 3, "window {h}");
    }
}

/// A slow page load fires no event: each checked window is read once more,
/// `LATE_WARM_UP` after the step's trigger.
#[test]
fn a_late_warm_up_reads_each_window_again_after_the_trigger() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let windows: Vec<_> = (1..=2).map(|h| (h, vec![WHOLE])).collect();
    let trigger_at = Instant::now();
    c.check(&windows, trigger_at, trigger_at, false);
    for (h, _) in &windows {
        assert_eq!(
            completions_after(&c, *h, trigger_at + LATE_WARM_UP, 2),
            2,
            "window {h}"
        );
    }
}

/// Enter in the password field: the page drops the field from its tree at
/// once, and a query completes empty between the trigger and the pixel read
/// while the old pixels are still on screen. The screenshot keeps the
/// cached field masked, and the change is flagged.
#[test]
fn an_empty_query_between_the_trigger_and_the_read_does_not_unmask_the_field() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![FIELD]);
    let now = Instant::now();
    let trigger_at = now - Duration::from_millis(50);
    completed(&c, W, now - Duration::from_millis(30), vec![]);
    let out = c.check(
        &one_window(),
        trigger_at,
        now - Duration::from_millis(10),
        false,
    );
    assert_eq!(out.masks, vec![FIELD]);
    assert!(
        out.flags
            .iter()
            .any(|f| f.contains("changed since the last check")),
        "{:?}",
        out.flags
    );
}

/// Steps in quick succession fill the cache with the warm-ups of the page
/// after a submitted login. They must not push out the completion from
/// before the trigger that still holds the field the pixels show.
#[test]
fn warm_ups_after_the_trigger_do_not_push_the_field_out_of_the_cache() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    warmed(&c, W, vec![FIELD]);
    let trigger_at = Instant::now();
    // Two steps' worth: each leaves its own query and two warm-ups of two.
    for _ in 0..10 {
        c.run_query(W, BUDGET);
    }
    let out = c.check(&one_window(), trigger_at, Instant::now(), false);
    assert_eq!(out.masks, vec![FIELD]);
}
