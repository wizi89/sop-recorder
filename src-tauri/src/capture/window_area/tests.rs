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
        partial: false,
    }
}

fn only(hwnd: isize, rect: Rect) -> Area {
    Area::Window {
        crop: rect,
        keep: vec![rect],
        windows: vec![hwnd],
    }
}

#[test]
fn a_click_inside_a_normal_window_keeps_that_window_only() {
    let s = snapshot(EDITOR, Some(EDITOR), desktop());
    assert_eq!(
        capture_area(&s, Some((300, 300)), MONITOR),
        only(EDITOR, Rect::new(100, 100, 900, 700))
    );
}

#[test]
fn a_key_press_is_placed_in_the_middle_of_the_foreground_window() {
    let mut windows = desktop();
    windows.push(window(9, Rect::new(1920, 0, 3840, 1080), "Notepad", 40));
    let s = snapshot(9, None, windows);
    assert_eq!(key_press_point(&s), Some((2880, 540)));
}

#[test]
fn a_key_press_on_the_desktop_or_without_a_foreground_window_has_no_point() {
    assert_eq!(key_press_point(&snapshot(DESKTOP, None, desktop())), None);
    let mut s = snapshot(EDITOR, None, desktop());
    s.foreground = None;
    assert_eq!(key_press_point(&s), None);
}

#[test]
fn a_key_press_keeps_the_foreground_window() {
    let s = snapshot(BROWSER, None, desktop());
    assert_eq!(
        capture_area(&s, None, MONITOR),
        only(BROWSER, Rect::new(800, 50, 1800, 900))
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

    let Area::Window { crop, keep, .. } = capture_area(&s, Some((1500, 700)), MONITOR) else {
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
        only(EDITOR, Rect::new(0, 900, 600, 1080))
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
        only(TASKBAR, Rect::new(0, 1032, 1920, 1080))
    );
}

/// Click-to-activate: the hook fires before the clicked window comes to the
/// front, so the window under the click is the one the step is about.
#[test]
fn a_click_on_a_background_window_keeps_that_window() {
    let s = snapshot(EDITOR, Some(BROWSER), desktop());
    assert_eq!(
        capture_area(&s, Some((1500, 400)), MONITOR),
        only(BROWSER, Rect::new(800, 50, 1800, 900))
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

    let Area::Window { crop, keep, .. } = capture_area(&s, Some((1000, 800)), MONITOR) else {
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
        only(MENU, Rect::new(1500, 900, 1900, 1000))
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
        only(EDITOR, Rect::new(100, 100, 900, 700))
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

mod masking;
