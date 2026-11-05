//! The password check of a step (privacy-capture D4-D6, amendments A1, A2).
//!
//! Platform-free: the query itself is a `FieldQuery`, which is UI Automation
//! on Windows (`capture::uia`) and a scripted closure in the tests, so every
//! timing and flag rule here is tested without a desktop.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use super::window_area::{visible_fields, Rect};

/// Per-step budget in the active window mode: one to three windows, a first
/// query on Chromium up to about 400 ms (spike, design.md).
pub const ACTIVE_WINDOW_BUDGET: Duration = Duration::from_millis(500);
/// Per-step budget in monitor mode: a 14-window desktop took up to 1.75 s.
pub const MONITOR_BUDGET: Duration = Duration::from_millis(2000);
/// Queries past their deadline that may still be running before steps skip the
/// check and warm-ups stop (D6). A step never queries a window whose earlier
/// query is past its deadline, so the threads stuck in a hung window stop
/// growing once its first query is overdue, and once this many are stuck
/// nothing new is started.
pub const ABANDONED_CAP: usize = 2;
/// How long a warm-up query may run before it counts as abandoned. Longer than
/// a step's budget: at recording start every visible window is warmed up at
/// once on one UIA client, and a 14-window pass took up to 1.75 s in the
/// spike. Under the step budget, ordinary slow warm-ups counted as hanging
/// and the first steps skipped their check (review of #26).
pub const WARM_UP_DEADLINE: Duration = Duration::from_secs(5);
/// A window counts as checked after two completed queries this far apart
/// (A2): Chromium's first query on a new window is complete but empty.
pub const WARM_UP_GAP: Duration = Duration::from_millis(100);

/// What one query of one window found.
#[derive(Debug, Clone, PartialEq)]
pub enum QueryResult {
    /// Password field rectangles, in desktop coordinates.
    Fields(Vec<Rect>),
    /// The query failed; the window may hold fields nobody saw.
    Failed(String),
    /// The window is not queried at all (A1): an elevated process answers with
    /// no fields and no error, which would read as "checked, nothing there".
    NotCheckable(&'static str),
    /// The window closed after the pixels were read, so nothing in it can have
    /// changed since: what was cached stands, and a window that was never
    /// checked is flagged as such already (8.1 run: closed windows flagged
    /// two steps in five on a real desktop).
    Gone,
}

/// One blocking query of one top-level window.
pub trait FieldQuery: Send + Sync {
    fn query(&self, hwnd: isize) -> QueryResult;
}

impl<F: Fn(isize) -> QueryResult + Send + Sync> FieldQuery for F {
    fn query(&self, hwnd: isize) -> QueryResult {
        self(hwnd)
    }
}

/// Completed queries kept per window. Each warm-up adds two, so this leaves
/// room for a few to land between a step's pixel read and its check without
/// pushing out the completions from before the read.
const KEPT_COMPLETIONS: usize = 8;

/// What the recorder knows about one window's password fields (D5).
///
/// Every result is kept with its time, never overwritten: a step's check needs
/// what was known when its pixels were read, and a query that finishes after
/// the read (a submitted login, the field gone) must not erase the field the
/// pixels still show.
#[derive(Debug, Clone, Default)]
struct Entry {
    /// The latest completed queries and what each found, oldest first.
    completions: Vec<(Instant, Vec<Rect>)>,
    /// Fields reported by focus, with when.
    focused: Vec<(Instant, Rect)>,
}

impl Entry {
    /// Two completed queries at least `WARM_UP_GAP` apart before `t` (A2).
    fn checked_before(&self, t: Instant) -> bool {
        Self::checked_between(&self.completions, None, t)
    }

    /// Two completed queries at least `WARM_UP_GAP` apart, after `since`
    /// (when given) and not after `t`.
    fn checked_between(
        completions: &[(Instant, Vec<Rect>)],
        since: Option<Instant>,
        t: Instant,
    ) -> bool {
        let done: Vec<Instant> = completions
            .iter()
            .map(|(c, _)| *c)
            .filter(|c| *c <= t && since.is_none_or(|s| *c > s))
            .collect();
        match (done.first(), done.last()) {
            (Some(first), Some(last)) => done.len() >= 2 && *last - *first >= WARM_UP_GAP,
            _ => false,
        }
    }

    /// Every field that may be on pixels read at `t`: the last query before
    /// `t`, every query after it (a field that showed up later is masked too;
    /// masking too much is safe), and each focused field until two queries
    /// after its focus, both before `t`, have found it gone.
    fn fields_at(&self, t: Instant) -> Vec<Rect> {
        let mut fields: Vec<Rect> = Vec::new();
        let mut add = |f: &Rect| {
            if !fields.contains(f) {
                fields.push(*f);
            }
        };
        let last_before = self.completions.iter().rposition(|(c, _)| *c <= t);
        for (i, (c, found)) in self.completions.iter().enumerate() {
            if Some(i) == last_before || *c > t {
                found.iter().for_each(&mut add);
            }
        }
        for (at, field) in &self.focused {
            let confirmed_gone = Self::checked_between(&self.completions, Some(*at), t)
                && self
                    .completions
                    .iter()
                    .filter(|(c, _)| *c > *at && *c <= t)
                    .all(|(_, f)| !f.contains(field));
            if !confirmed_gone {
                add(field);
            }
        }
        fields
    }
}

/// The outcome of one step's check.
#[derive(Debug, Default, PartialEq)]
pub struct CheckOutcome {
    /// Visible parts of every known field, to fill before the PNG is written.
    pub masks: Vec<Rect>,
    /// Why the step is flagged; empty means checked and `ok`.
    pub flags: Vec<String>,
}

/// The per-recording password checker: cache, warm-ups and step checks, all
/// through one capped pool of running queries.
pub struct Checker {
    query: Arc<dyn FieldQuery>,
    budget: Duration,
    cache: Mutex<HashMap<isize, Entry>>,
    /// Every query still running: id, window, deadline.
    running: Mutex<Vec<(u64, isize, Instant)>>,
    next_id: AtomicU64,
}

impl Checker {
    pub fn new(query: Arc<dyn FieldQuery>, budget: Duration) -> Arc<Self> {
        Arc::new(Self {
            query,
            budget,
            cache: Mutex::new(HashMap::new()),
            running: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(0),
        })
    }

    /// Queries still running past their deadline (D6). They cannot be
    /// cancelled, only abandoned; each one is a thread the hung app holds.
    fn abandoned(&self) -> usize {
        let now = Instant::now();
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(_, _, deadline)| *deadline < now)
            .count()
    }

    /// A query of this window is still running past its deadline: the window
    /// is not answering, and another query would only be stuck beside it.
    fn hung(&self, hwnd: isize) -> bool {
        let now = Instant::now();
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|(_, h, deadline)| *h == hwnd && *deadline < now)
    }

    /// One query, registered while it runs and recorded in the cache when it
    /// completes with fields. It counts as abandoned once `allowed` has passed.
    fn run_query(&self, hwnd: isize, allowed: Duration) -> QueryResult {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((id, hwnd, Instant::now() + allowed));
        let result = self.query.query(hwnd);
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|(i, _, _)| *i != id);
        if let QueryResult::Fields(fields) = &result {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            let entry = cache.entry(hwnd).or_default();
            entry.completions.push((Instant::now(), fields.clone()));
            if entry.completions.len() > KEPT_COMPLETIONS {
                entry.completions.remove(0);
            }
        }
        result
    }

    /// Query a window twice, `WARM_UP_GAP` apart, so a step on it is checked
    /// (D5, A2). Runs under the same cap as step checks, and not twice at once
    /// for the same window.
    pub fn warm_up(self: &Arc<Self>, hwnd: isize) {
        if self.abandoned() >= ABANDONED_CAP {
            return;
        }
        let busy = self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|(_, h, _)| *h == hwnd);
        if busy {
            return;
        }
        let checker = self.clone();
        std::thread::spawn(move || {
            if let QueryResult::Fields(_) = checker.run_query(hwnd, WARM_UP_DEADLINE) {
                std::thread::sleep(WARM_UP_GAP);
                checker.run_query(hwnd, WARM_UP_DEADLINE);
            }
        });
    }

    /// A password field received focus (D5). Typing a password needs focus,
    /// so this catches the field a page inserted without any window event.
    pub fn note_focused_field(&self, hwnd: isize, field: Rect) {
        if field.is_empty() {
            return;
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let focused = &mut cache.entry(hwnd).or_default().focused;
        if !focused.iter().any(|(_, f)| *f == field) {
            focused.push((Instant::now(), field));
        }
    }

    /// Check one step: `windows` are those visible in its saved area with
    /// their visible parts (D4), `pixels_at` is when its pixels were read.
    ///
    /// Masks the union of cached and fresh fields (D5) and flags every case
    /// where a field may have been missed.
    pub fn check(
        self: &Arc<Self>,
        windows: &[(isize, Vec<Rect>)],
        pixels_at: Instant,
        foreground_changed: bool,
    ) -> CheckOutcome {
        let mut flags = Vec::new();
        if foreground_changed {
            flags.push("the foreground window changed while the pixels were read".to_string());
        }

        // Taken before the fresh queries write to the cache. What it knew at
        // `pixels_at` comes from `fields_at`, not from the newest result:
        // queries keep completing between the read and this check.
        let cached: HashMap<isize, Entry> = {
            let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            windows
                .iter()
                .map(|(h, _)| (*h, cache.get(h).cloned().unwrap_or_default()))
                .collect()
        };
        for (hwnd, _) in windows {
            if !cached[hwnd].checked_before(pixels_at) {
                flags.push(format!(
                    "window {hwnd:#x} had not been checked twice before the step"
                ));
            }
        }

        let mut fresh: HashMap<isize, Vec<Rect>> = HashMap::new();
        if self.abandoned() >= ABANDONED_CAP {
            flags.push(format!(
                "{ABANDONED_CAP} earlier checks are still hanging; this step was not checked"
            ));
        } else if !windows.is_empty() {
            // One thread per window, so the budget covers the slowest window
            // rather than the sum: in the 8.1 run two Chrome windows at ~400 ms
            // each ran a step past 500 ms one after the other.
            let (tx, rx) = mpsc::channel();
            let mut asked = 0;
            for (hwnd, _) in windows {
                if self.hung(*hwnd) {
                    flags.push(format!(
                        "window {hwnd:#x} is not answering an earlier check; this step did not check it"
                    ));
                    continue;
                }
                let (tx, checker, hwnd, budget) = (tx.clone(), self.clone(), *hwnd, self.budget);
                std::thread::spawn(move || {
                    let _ = tx.send((hwnd, checker.run_query(hwnd, budget)));
                });
                asked += 1;
            }

            let deadline = Instant::now() + self.budget;
            for _ in 0..asked {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok((hwnd, QueryResult::Fields(found))) => {
                        if found.iter().any(Rect::is_empty) {
                            flags.push(format!("window {hwnd:#x} has a field still being built"));
                        }
                        fresh.insert(hwnd, found.into_iter().filter(|r| !r.is_empty()).collect());
                    }
                    Ok((hwnd, QueryResult::Failed(e))) => {
                        flags.push(format!("window {hwnd:#x} could not be checked: {e}"));
                    }
                    Ok((_, QueryResult::Gone)) => {}
                    Ok((hwnd, QueryResult::NotCheckable(why))) => {
                        flags.push(format!("window {hwnd:#x} is not checked: {why}"));
                    }
                    Err(_) => {
                        // The queries still running are abandoned and count
                        // against the cap.
                        flags.push(format!(
                            "the check ran out of its {} ms budget",
                            self.budget.as_millis()
                        ));
                        break;
                    }
                }
            }
        }

        let mut masks = Vec::new();
        for (hwnd, visible) in windows {
            let mut fields = cached[hwnd].fields_at(pixels_at);
            if let Some(found) = fresh.get(hwnd) {
                let same = found.len() == fields.len() && found.iter().all(|f| fields.contains(f));
                if !same {
                    flags.push(format!(
                        "window {hwnd:#x}: password fields changed since the last check"
                    ));
                }
                fields.extend(
                    found
                        .iter()
                        .filter(|f| !fields.contains(f))
                        .copied()
                        .collect::<Vec<_>>(),
                );
            }
            masks.extend(visible_fields(&fields, visible));
        }
        CheckOutcome { masks, flags }
    }

    /// Completions and fields per window, for the live test's printout.
    #[cfg(test)]
    pub fn debug_cache(&self) -> Vec<(isize, usize, Vec<Rect>)> {
        self.cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(h, e)| (*h, e.completions.len(), e.fields_at(Instant::now())))
            .collect()
    }

    #[cfg(test)]
    fn seed(&self, hwnd: isize, fields: Vec<Rect>, completions: Vec<Instant>) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).insert(
            hwnd,
            Entry {
                completions: completions
                    .into_iter()
                    .map(|c| (c, fields.clone()))
                    .collect(),
                focused: Vec::new(),
            },
        );
    }
}

/// Keeps the warm-up triggers of a recording running (D5); dropping it stops
/// them. Off Windows there are none, and nothing ever constructs one.
#[cfg(windows)]
pub use super::uia::Watch;

#[cfg(not(windows))]
pub struct Watch;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

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
    /// take 160% of the budget one after another fit into it together.
    #[test]
    fn windows_are_checked_side_by_side_within_one_budget() {
        let c = checker(|_| {
            std::thread::sleep(BUDGET * 4 / 10);
            QueryResult::Fields(vec![])
        });
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
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();
        let c = checker(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(50));
            QueryResult::Fields(vec![])
        });
        c.warm_up(W);
        std::thread::sleep(Duration::from_millis(10));
        c.warm_up(W);
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
