//! #66: what a step's check leaves behind for the next step.

use super::*;

/// #66: a page navigated after a step's check (a submitted login, a tab
/// switch). The warm-up after the check reads the new page, so the next
/// step is checked against it: no stale mask, no "changed" flag.
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
    let first = c.check(&one_window(), Instant::now(), false);
    assert_eq!(first.masks, vec![FIELD]);
    std::thread::sleep(WARM_UP_GAP * 3);
    let next = c.check(&one_window(), Instant::now(), false);
    assert_eq!(
        next,
        CheckOutcome {
            masks: vec![],
            flags: vec![]
        }
    );
}

/// #66: a step's check warms up every window it checked, so the cache
/// holds two completions from after the step for the next one.
#[test]
fn a_check_leaves_two_completions_after_the_step_for_each_window() {
    let c = checker(|_| QueryResult::Fields(vec![]));
    let windows: Vec<_> = (1..=2).map(|h| (h, vec![WHOLE])).collect();
    let pixels_at = Instant::now();
    c.check(&windows, pixels_at, false);
    std::thread::sleep(WARM_UP_GAP * 3);
    let cache = c.cache.lock().unwrap_or_else(|e| e.into_inner());
    for (h, _) in &windows {
        let after = cache[h]
            .completions
            .iter()
            .filter(|(at, _)| *at > pixels_at)
            .count();
        // The step's own query plus the warm-up's two.
        assert!(after >= 3, "window {h}: {after} completions after the step");
    }
}
