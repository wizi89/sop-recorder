//! The part of the monitor a step keeps in the active window mode
//! (privacy-capture D2), and the window geometry it is computed from (D3).
//!
//! Pure on purpose: the Windows reads that fill a `Snapshot` live in
//! `capture::geometry`, so every rule here is tested on any platform against
//! desktops written out by hand.

/// A rectangle in desktop coordinates (physical pixels on Windows), right and
/// bottom exclusive, as `RECT` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let r = Rect::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        );
        (!r.is_empty()).then_some(r)
    }

    /// What is left of `self` once `cut` is taken out: up to four pieces.
    pub fn subtract(&self, cut: &Rect) -> Vec<Rect> {
        let Some(hole) = self.intersect(cut) else {
            return vec![*self];
        };
        [
            Rect::new(self.left, self.top, self.right, hole.top),
            Rect::new(self.left, hole.bottom, self.right, self.bottom),
            Rect::new(self.left, hole.top, hole.left, hole.bottom),
            Rect::new(hole.right, hole.top, self.right, hole.bottom),
        ]
        .into_iter()
        .filter(|r| !r.is_empty())
        .collect()
    }

    pub fn bounding(&self, other: &Rect) -> Rect {
        Rect::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }
}

/// One visible top-level window, as the geometry snapshot saw it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WindowInfo {
    pub hwnd: isize,
    /// DWM extended frame bounds: the window as drawn, without the invisible
    /// resize border.
    pub rect: Rect,
    pub class: String,
    pub pid: u32,
    /// `WS_POPUP`: menus, dropdowns and other windows drawn over their owner.
    pub popup: bool,
    /// `GetAncestor(GA_ROOTOWNER)`; the window itself when it has no owner.
    pub root_owner: isize,
    /// Not drawn at all (another virtual desktop, a suspended app).
    pub cloaked: bool,
    /// `WS_EX_LAYERED` or `WS_EX_TRANSPARENT`: may let what is below show
    /// through, so it never counts as hiding anything (D4).
    pub layered: bool,
    pub transparent: bool,
    /// Display affinity other than `WDA_NONE`: absent from the pixels.
    pub excluded: bool,
}

impl Default for Rect {
    fn default() -> Self {
        Rect::new(0, 0, 0, 0)
    }
}

/// Foreground window, window under the click and every visible top-level
/// window, top of the z-order first (D3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub foreground: Option<isize>,
    /// The root window under the click; `None` for a key press.
    pub under_click: Option<isize>,
    pub windows: Vec<WindowInfo>,
}

impl Snapshot {
    pub fn get(&self, hwnd: isize) -> Option<&WindowInfo> {
        self.windows.iter().find(|w| w.hwnd == hwnd)
    }
}

/// The two snapshots taken either side of the pixel read, as the password
/// check needs them (D3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Geometry {
    /// Before and after. Visibility is worked out in each and combined: a
    /// window counts where it is visible in either, because the pixels may
    /// show either moment.
    pub snapshots: [Snapshot; 2],
    /// Activation completed during the pixel read, so the pixels may show
    /// either foreground window. Flags the step when masking is on.
    pub foreground_changed: bool,
}

pub fn merge(before: &Snapshot, after: &Snapshot) -> Geometry {
    Geometry {
        foreground_changed: before.foreground != after.foreground,
        snapshots: [before.clone(), after.clone()],
    }
}

/// Whether a window certainly hides what is below it (D4). Anything that may
/// let the lower window show through counts as not hiding it, so an error
/// here over-masks rather than leaks.
fn hides(w: &WindowInfo) -> bool {
    !w.cloaked && !w.layered && !w.transparent && !w.excluded
}

/// Whether a window's pixels can be in the screenshot at all: a cloaked window
/// is not drawn and a window excluded from capture (the recording bar) is not
/// in the pixels, so neither is checked.
fn drawn(w: &WindowInfo) -> bool {
    !w.cloaked && !w.excluded
}

/// Each drawn window with its visible part inside `shown`: its rectangle minus
/// every window above it that certainly hides it (D4). Windows with no visible
/// part are left out.
fn visible_in(snapshot: &Snapshot, shown: &[Rect]) -> Vec<(isize, Vec<Rect>)> {
    let mut out = Vec::new();
    for (i, w) in snapshot.windows.iter().enumerate() {
        if !drawn(w) {
            continue;
        }
        let mut parts: Vec<Rect> = shown.iter().filter_map(|s| w.rect.intersect(s)).collect();
        for above in snapshot.windows[..i].iter().filter(|a| hides(a)) {
            parts = parts.iter().flat_map(|p| p.subtract(&above.rect)).collect();
        }
        // A `#32768` menu still hides what is below it, but holds no input
        // field, and is usually closed by the click before the check runs.
        if !parts.is_empty() && w.class != "#32768" {
            out.push((w.hwnd, parts));
        }
    }
    out
}

/// The windows the password check covers and where each is visible in the
/// saved area, top of the z-order first (D4). Visible in either snapshot
/// counts as visible (D3).
pub fn visible_windows(geometry: &Geometry, shown: &[Rect]) -> Vec<(isize, Vec<Rect>)> {
    let mut out = visible_in(&geometry.snapshots[0], shown);
    for (hwnd, parts) in visible_in(&geometry.snapshots[1], shown) {
        match out.iter_mut().find(|(h, _)| *h == hwnd) {
            Some((_, seen)) => {
                for part in parts {
                    if !seen.contains(&part) {
                        seen.push(part);
                    }
                }
            }
            None => out.push((hwnd, parts)),
        }
    }
    out
}

/// The parts of `fields` that are visible: masked only there, so a password
/// field of a covered window does not black out the window in front (D4).
pub fn visible_fields(fields: &[Rect], visible: &[Rect]) -> Vec<Rect> {
    fields
        .iter()
        .flat_map(|f| visible.iter().filter_map(|v| f.intersect(v)))
        .collect()
}

/// What a step keeps of its monitor.
#[derive(Debug, Clone, PartialEq)]
pub enum Area {
    /// Crop to `crop`; pixels inside it but outside every `keep` rectangle are
    /// filled, so the gap between a dialog and a distant owner shows nothing.
    Window { crop: Rect, keep: Vec<Rect> },
    /// No usable window: the whole monitor is kept and the step is marked.
    Fallback(&'static str),
}

#[derive(Debug, PartialEq)]
enum Kind {
    /// `Progman` / `WorkerW`: counts as no window at all.
    Desktop,
    /// Taskbar, Start, tray flyouts: captured alone when clicked.
    Shell,
    App,
}

const DESKTOP_CLASSES: [&str; 2] = ["Progman", "WorkerW"];
const SHELL_CLASSES: [&str; 7] = [
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "Shell_InputSwitchTopLevelWindow",
];

fn kind(w: &WindowInfo) -> Kind {
    if DESKTOP_CLASSES.contains(&w.class.as_str()) {
        Kind::Desktop
    } else if SHELL_CLASSES.contains(&w.class.as_str()) {
        Kind::Shell
    } else {
        Kind::App
    }
}

/// A context menu, or a popup of the foreground window's process reaching past
/// its edge (a dropdown): shown together with the window it belongs to.
fn belongs_to(popup: &WindowInfo, fg: &WindowInfo) -> bool {
    popup.class == "#32768" || (popup.popup && popup.pid == fg.pid && popup.hwnd != fg.hwnd)
}

/// The area a step keeps in the active window mode (D2).
///
/// The foreground window and its root owner, unless the click lands outside
/// both: then the root window under the click decides. A click fires before
/// its own activation, so clicking a background window has to capture that
/// window, not the one about to lose the foreground.
pub fn capture_area(snapshot: &Snapshot, click: Option<(i32, i32)>, monitor: Rect) -> Area {
    // A cloaked window is in the list but never drawn (a closed Start menu,
    // another virtual desktop), so it can be no step's window.
    let drawn = |hwnd: isize| snapshot.get(hwnd).filter(|w| !w.cloaked);
    let with_owner = |w: &WindowInfo| -> Vec<Rect> {
        let mut rects = vec![w.rect];
        if let Some(owner) = drawn(w.root_owner).filter(|o| o.hwnd != w.hwnd) {
            rects.push(owner.rect);
        }
        rects
    };
    let window_rects = |w: &WindowInfo| -> Option<Vec<Rect>> {
        match kind(w) {
            Kind::Desktop => None,
            Kind::Shell => Some(vec![w.rect]),
            Kind::App => Some(with_owner(w)),
        }
    };

    let fg = snapshot.foreground.and_then(drawn);
    let fg_rects = fg.and_then(window_rects);

    let rects = match click {
        None => match fg_rects {
            Some(rects) => rects,
            None => return Area::Fallback("no foreground window"),
        },
        Some(point) if fg_rects.iter().flatten().any(|r| r.contains(point)) => fg_rects.unwrap(),
        Some(_) => {
            let Some(under) = snapshot.under_click.and_then(drawn) else {
                return Area::Fallback("no window under the click");
            };
            match (fg, &fg_rects) {
                (Some(fg), Some(fg_rects)) if kind(under) == Kind::App && belongs_to(under, fg) => {
                    let mut rects = fg_rects.clone();
                    rects.push(under.rect);
                    rects
                }
                _ => match window_rects(under) {
                    Some(rects) => rects,
                    None => return Area::Fallback("click on the desktop"),
                },
            }
        }
    };

    let keep: Vec<Rect> = rects.iter().filter_map(|r| r.intersect(&monitor)).collect();
    let Some(crop) = keep.iter().copied().reduce(|a, b| a.bounding(&b)) else {
        return Area::Fallback("window is not on the captured monitor");
    };
    Area::Window { crop, keep }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect::new(0, 0, 1920, 1080);

    fn window(hwnd: isize, rect: Rect, class: &str, pid: u32) -> WindowInfo {
        WindowInfo {
            hwnd,
            rect,
            class: class.into(),
            pid,
            root_owner: hwnd,
            ..Default::default()
        }
    }

    const EDITOR: isize = 1;
    const DIALOG: isize = 2;
    const BROWSER: isize = 3;
    const TASKBAR: isize = 4;
    const DESKTOP: isize = 5;
    const MENU: isize = 6;

    /// An editor, a browser behind it, the taskbar and the desktop.
    fn desktop() -> Vec<WindowInfo> {
        vec![
            window(EDITOR, Rect::new(100, 100, 900, 700), "Notepad", 10),
            window(
                BROWSER,
                Rect::new(800, 50, 1800, 900),
                "Chrome_WidgetWin_1",
                20,
            ),
            window(TASKBAR, Rect::new(0, 1032, 1920, 1080), "Shell_TrayWnd", 30),
            window(DESKTOP, MONITOR, "Progman", 30),
        ]
    }

    fn snapshot(fg: isize, under: Option<isize>, windows: Vec<WindowInfo>) -> Snapshot {
        Snapshot {
            foreground: Some(fg),
            under_click: under,
            windows,
        }
    }

    fn only(rect: Rect) -> Area {
        Area::Window {
            crop: rect,
            keep: vec![rect],
        }
    }

    #[test]
    fn a_click_inside_a_normal_window_keeps_that_window_only() {
        let s = snapshot(EDITOR, Some(EDITOR), desktop());
        assert_eq!(
            capture_area(&s, Some((300, 300)), MONITOR),
            only(Rect::new(100, 100, 900, 700))
        );
    }

    #[test]
    fn a_key_press_keeps_the_foreground_window() {
        let s = snapshot(BROWSER, None, desktop());
        assert_eq!(
            capture_area(&s, None, MONITOR),
            only(Rect::new(800, 50, 1800, 900))
        );
    }

    /// The gap between a dialog and a distant owner is inside the crop but in
    /// neither rectangle, so it is filled rather than shown.
    #[test]
    fn a_dialog_away_from_its_owner_keeps_both_and_the_gap_is_not_kept() {
        let mut windows = desktop();
        let mut dialog = window(DIALOG, Rect::new(1400, 600, 1800, 900), "#32770", 10);
        dialog.root_owner = EDITOR;
        windows.insert(0, dialog);
        let s = snapshot(DIALOG, Some(DIALOG), windows);

        let Area::Window { crop, keep } = capture_area(&s, Some((1500, 700)), MONITOR) else {
            panic!("expected a window area");
        };
        assert_eq!(crop, Rect::new(100, 100, 1800, 900));
        assert_eq!(
            keep,
            vec![
                Rect::new(1400, 600, 1800, 900),
                Rect::new(100, 100, 900, 700)
            ]
        );
        let gap = (1100, 300);
        assert!(crop.contains(gap) && !keep.iter().any(|r| r.contains(gap)));
    }

    #[test]
    fn a_window_partly_off_screen_keeps_only_its_visible_part() {
        let windows = vec![window(
            EDITOR,
            Rect::new(-300, 900, 600, 1400),
            "Notepad",
            10,
        )];
        let s = snapshot(EDITOR, Some(EDITOR), windows);
        assert_eq!(
            capture_area(&s, Some((100, 1000)), MONITOR),
            only(Rect::new(0, 900, 600, 1080))
        );
    }

    #[test]
    fn a_click_on_the_desktop_falls_back_to_the_monitor() {
        let s = snapshot(EDITOR, Some(DESKTOP), desktop());
        assert_eq!(
            capture_area(&s, Some((50, 50)), MONITOR),
            Area::Fallback("click on the desktop")
        );
    }

    #[test]
    fn the_desktop_in_front_with_no_click_falls_back() {
        let s = snapshot(DESKTOP, None, desktop());
        assert_eq!(
            capture_area(&s, None, MONITOR),
            Area::Fallback("no foreground window")
        );
    }

    #[test]
    fn a_click_on_the_taskbar_keeps_the_taskbar_only() {
        let s = snapshot(EDITOR, Some(TASKBAR), desktop());
        assert_eq!(
            capture_area(&s, Some((500, 1050)), MONITOR),
            only(Rect::new(0, 1032, 1920, 1080))
        );
    }

    /// Click-to-activate: the hook fires before the clicked window comes to the
    /// front, so the window under the click is the one the step is about.
    #[test]
    fn a_click_on_a_background_window_keeps_that_window() {
        let s = snapshot(EDITOR, Some(BROWSER), desktop());
        assert_eq!(
            capture_area(&s, Some((1500, 400)), MONITOR),
            only(Rect::new(800, 50, 1800, 900))
        );
    }

    #[test]
    fn a_context_menu_reaching_past_the_window_edge_is_kept_with_the_window() {
        let mut windows = desktop();
        windows.insert(
            0,
            window(MENU, Rect::new(850, 600, 1100, 950), "#32768", 10),
        );
        let s = snapshot(EDITOR, Some(MENU), windows);

        let Area::Window { crop, keep } = capture_area(&s, Some((1000, 800)), MONITOR) else {
            panic!("expected a window area");
        };
        assert_eq!(crop, Rect::new(100, 100, 1100, 950));
        assert_eq!(
            keep,
            vec![
                Rect::new(100, 100, 900, 700),
                Rect::new(850, 600, 1100, 950)
            ]
        );
    }

    /// A dropdown is not a `#32768` menu but a popup of the same process.
    #[test]
    fn a_dropdown_of_the_same_process_is_kept_with_the_window() {
        let mut windows = desktop();
        let mut dropdown = window(
            MENU,
            Rect::new(700, 650, 950, 800),
            "Chrome_WidgetWin_1",
            10,
        );
        dropdown.popup = true;
        windows.insert(0, dropdown);
        let s = snapshot(EDITOR, Some(MENU), windows);

        let Area::Window { keep, .. } = capture_area(&s, Some((920, 750)), MONITOR) else {
            panic!("expected a window area");
        };
        assert_eq!(keep.len(), 2);
    }

    /// A popup of another process is another window, not a menu of this one.
    #[test]
    fn a_popup_of_another_process_replaces_the_foreground_window() {
        let mut windows = desktop();
        let mut toast = window(MENU, Rect::new(1500, 900, 1900, 1000), "Toast", 99);
        toast.popup = true;
        windows.insert(0, toast);
        let s = snapshot(EDITOR, Some(MENU), windows);
        assert_eq!(
            capture_area(&s, Some((1600, 950)), MONITOR),
            only(Rect::new(1500, 900, 1900, 1000))
        );
    }

    #[test]
    fn a_window_on_another_monitor_falls_back() {
        let windows = vec![window(EDITOR, Rect::new(2000, 0, 2500, 500), "Notepad", 10)];
        let s = snapshot(EDITOR, None, windows);
        assert_eq!(
            capture_area(&s, None, MONITOR),
            Area::Fallback("window is not on the captured monitor")
        );
    }

    /// A hidden owner (common: many apps own their windows from an invisible
    /// one) is not in the snapshot, so only the window itself is kept.
    #[test]
    fn an_invisible_owner_adds_nothing() {
        let mut windows = desktop();
        windows[0].root_owner = 777;
        let s = snapshot(EDITOR, Some(EDITOR), windows);
        assert_eq!(
            capture_area(&s, Some((300, 300)), MONITOR),
            only(Rect::new(100, 100, 900, 700))
        );
    }

    #[test]
    fn a_cloaked_window_is_never_the_steps_window() {
        let mut windows = desktop();
        let mut start = window(MENU, MONITOR, "Windows.UI.Core.CoreWindow", 30);
        start.cloaked = true;
        windows.insert(0, start);
        let s = snapshot(EDITOR, Some(MENU), windows);
        assert_eq!(
            capture_area(&s, Some((1000, 800)), MONITOR),
            Area::Fallback("no window under the click")
        );
    }

    // ------------------------------------------------------------- task 3.3

    fn hwnds(windows: &[(isize, Vec<Rect>)]) -> Vec<isize> {
        windows.iter().map(|(h, _)| *h).collect()
    }

    #[test]
    fn a_window_visible_in_either_snapshot_counts_as_visible() {
        let before = snapshot(EDITOR, Some(EDITOR), desktop());
        let mut after = before.clone();
        after.windows.retain(|w| w.hwnd != BROWSER);
        let toast = window(MENU, Rect::new(10, 10, 50, 50), "Toast", 99);
        after.windows.insert(0, toast);

        let visible = hwnds(&visible_windows(&merge(&before, &after), &[MONITOR]));
        assert!(visible.contains(&BROWSER), "only in before");
        assert!(visible.contains(&MENU), "only in after");
        assert!(!merge(&before, &after).foreground_changed);
    }

    /// A dialog that appears during the pixel read is on top in the second
    /// snapshot; it must not be computed as hidden behind its owner.
    #[test]
    fn a_window_that_appeared_during_the_read_is_visible_where_it_is_on_top() {
        let before = snapshot(EDITOR, Some(EDITOR), desktop());
        let mut after = before.clone();
        after.windows.insert(
            0,
            window(DIALOG, Rect::new(300, 300, 600, 500), "#32770", 10),
        );

        let visible = visible_windows(&merge(&before, &after), &[MONITOR]);
        let (_, parts) = visible.iter().find(|(h, _)| *h == DIALOG).unwrap();
        assert_eq!(parts, &vec![Rect::new(300, 300, 600, 500)]);
    }

    #[test]
    fn a_window_that_moved_is_visible_at_both_places() {
        let windows = vec![window(EDITOR, Rect::new(100, 100, 900, 700), "Notepad", 10)];
        let before = snapshot(EDITOR, Some(EDITOR), windows);
        let mut after = before.clone();
        after.windows[0].rect = Rect::new(200, 200, 1000, 800);

        let visible = visible_windows(&merge(&before, &after), &[MONITOR]);
        assert_eq!(
            visible,
            vec![(
                EDITOR,
                vec![
                    Rect::new(100, 100, 900, 700),
                    Rect::new(200, 200, 1000, 800)
                ]
            )]
        );
    }

    #[test]
    fn a_foreground_change_during_the_pixel_read_is_reported() {
        let before = snapshot(EDITOR, Some(BROWSER), desktop());
        let after = snapshot(BROWSER, Some(BROWSER), desktop());
        assert!(merge(&before, &after).foreground_changed);
    }

    // ------------------------------------------------------------- task 4.1

    /// A login window (the field at 300..500 x 300..330) with `cover` above it.
    fn field_behind(cover: WindowInfo) -> Vec<Rect> {
        let login = window(
            BROWSER,
            Rect::new(100, 100, 900, 700),
            "Chrome_WidgetWin_1",
            20,
        );
        let s = snapshot(EDITOR, None, vec![cover, login]);
        let visible = visible_windows(&merge(&s, &s), &[MONITOR]);
        let (_, parts) = visible
            .iter()
            .find(|(h, _)| *h == BROWSER)
            .cloned()
            .unwrap_or((BROWSER, Vec::new()));
        visible_fields(&[Rect::new(300, 300, 500, 330)], &parts)
    }

    fn cover() -> WindowInfo {
        window(EDITOR, Rect::new(50, 50, 950, 750), "Notepad", 10)
    }

    #[test]
    fn a_field_behind_an_opaque_window_is_not_masked() {
        assert_eq!(field_behind(cover()), vec![]);
    }

    /// Found in the 8.1 run: a context menu closed by the click had no
    /// process left to ask, which flagged the step for nothing.
    #[test]
    fn a_menu_is_not_checked_but_still_hides_what_is_below() {
        let menu = window(MENU, Rect::new(250, 250, 600, 400), "#32768", 20);
        let login = window(
            BROWSER,
            Rect::new(100, 100, 900, 700),
            "Chrome_WidgetWin_1",
            20,
        );
        let s = snapshot(BROWSER, Some(MENU), vec![menu, login]);
        let visible = visible_windows(&merge(&s, &s), &[MONITOR]);
        assert_eq!(hwnds(&visible), vec![BROWSER]);
        let fields = visible_fields(&[Rect::new(300, 300, 500, 330)], &visible[0].1);
        assert!(fields
            .iter()
            .all(|f| f.intersect(&Rect::new(250, 250, 600, 400)).is_none()));
    }

    #[test]
    fn a_field_behind_a_layered_window_is_masked() {
        let mut layered = cover();
        layered.layered = true;
        assert_eq!(field_behind(layered), vec![Rect::new(300, 300, 500, 330)]);
    }

    #[test]
    fn a_field_behind_a_transparent_window_is_masked() {
        let mut transparent = cover();
        transparent.transparent = true;
        assert_eq!(
            field_behind(transparent),
            vec![Rect::new(300, 300, 500, 330)]
        );
    }

    #[test]
    fn a_field_behind_a_cloaked_window_is_masked() {
        let mut cloaked = cover();
        cloaked.cloaked = true;
        assert_eq!(field_behind(cloaked), vec![Rect::new(300, 300, 500, 330)]);
    }

    /// The recording bar is excluded from capture when `hide_from_screenshots`
    /// is on: it is not in the pixels, so what is below it is.
    #[test]
    fn a_field_behind_a_window_excluded_from_capture_is_masked() {
        let mut bar = cover();
        bar.excluded = true;
        assert_eq!(field_behind(bar), vec![Rect::new(300, 300, 500, 330)]);
    }

    #[test]
    fn a_field_half_covered_is_masked_only_where_it_shows() {
        let half = window(EDITOR, Rect::new(400, 0, 1000, 1000), "Notepad", 10);
        assert_eq!(field_behind(half), vec![Rect::new(300, 300, 400, 330)]);
    }

    #[test]
    fn cloaked_and_excluded_windows_are_not_checked() {
        let mut cloaked = window(MENU, Rect::new(0, 0, 100, 100), "CoreWindow", 30);
        cloaked.cloaked = true;
        let mut bar = window(DIALOG, Rect::new(0, 0, 100, 100), "Bar", 40);
        bar.excluded = true;
        let s = snapshot(EDITOR, None, vec![cloaked, bar]);
        assert!(visible_windows(&merge(&s, &s), &[MONITOR]).is_empty());
    }

    /// Only what the step saves is checked: a window outside the kept area is
    /// left out even if it is on the monitor.
    #[test]
    fn a_window_outside_the_saved_area_is_not_checked() {
        let s = snapshot(EDITOR, None, desktop());
        let kept = [Rect::new(100, 100, 700, 700)];
        let visible = hwnds(&visible_windows(&merge(&s, &s), &kept));
        assert_eq!(visible, vec![EDITOR]);
    }

    #[test]
    fn subtract_leaves_the_frame_around_a_hole() {
        let pieces = Rect::new(0, 0, 10, 10).subtract(&Rect::new(3, 3, 6, 6));
        let area: i32 = pieces
            .iter()
            .map(|r| (r.right - r.left) * (r.bottom - r.top))
            .sum();
        assert_eq!(area, 100 - 9);
        assert!(pieces
            .iter()
            .all(|p| p.intersect(&Rect::new(3, 3, 6, 6)).is_none()));
    }
}
