use super::*;

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

#[test]
fn only_an_area_inside_shell_windows_is_shell_only() {
    let taskbar = Rect::new(0, 1368, 2560, 1440);
    let s = snapshot(
        EDITOR,
        None,
        vec![
            window(MENU, taskbar, "Shell_TrayWnd", 5),
            window(EDITOR, Rect::new(0, 0, 2560, 1368), "Notepad", 10),
        ],
    );
    assert!(shell_only(&s, &[taskbar]));
    assert!(!shell_only(&s, &[Rect::new(0, 0, 2560, 1368)]));
    assert!(!shell_only(&s, &[taskbar, Rect::new(0, 0, 100, 100)]));
    assert!(!shell_only(&s, &[]));
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

// ------------------------------------------- windows over the area

const TOAST: isize = 7;
const EDITOR_RECT: Rect = Rect::new(100, 100, 900, 700);

/// The toast host as the spike found it: a topmost shell popup of another
/// process, `Windows.UI.Core.CoreWindow`, over the editor's bottom right.
fn toast() -> WindowInfo {
    let mut t = window(
        TOAST,
        Rect::new(700, 500, 1300, 728),
        "Windows.UI.Core.CoreWindow",
        50,
    );
    t.popup = true;
    t
}

fn with_on_top(on_top: WindowInfo) -> Snapshot {
    let mut windows = desktop();
    windows.insert(0, on_top);
    snapshot(EDITOR, Some(EDITOR), windows)
}

fn step_windows(s: &Snapshot, click: Option<(i32, i32)>) -> Vec<isize> {
    match capture_area(s, click, MONITOR) {
        Area::Window { windows, .. } => windows,
        Area::Fallback(reason) => panic!("fell back: {reason}"),
    }
}

#[test]
fn a_toast_over_the_window_is_masked_where_it_overlaps() {
    let s = with_on_top(toast());
    let step = step_windows(&s, Some((300, 300)));
    assert_eq!(step, vec![EDITOR]);
    assert_eq!(
        foreign_rects(&[&s, &s], &step),
        vec![Rect::new(700, 500, 900, 700)]
    );
}

#[test]
fn a_toast_that_appears_during_the_read_is_masked() {
    let before = snapshot(EDITOR, Some(EDITOR), desktop());
    let after = with_on_top(toast());
    assert_eq!(
        foreign_rects(&[&before, &after], &[EDITOR]),
        vec![Rect::new(700, 500, 900, 700)]
    );
}

#[test]
fn a_cloaked_or_capture_excluded_window_is_not_masked() {
    let mut idle = toast();
    idle.cloaked = true;
    assert!(foreign_rects(&[&with_on_top(idle)], &[EDITOR]).is_empty());
    let mut bar = toast();
    bar.excluded = true;
    assert!(foreign_rects(&[&with_on_top(bar)], &[EDITOR]).is_empty());
}

#[test]
fn a_click_through_overlay_is_not_masked() {
    let mut overlay = window(TOAST, Rect::new(300, 200, 1000, 1000), "Overlay", 50);
    overlay.layered = true;
    overlay.transparent = true;
    overlay.click_through = true;
    assert!(foreign_rects(&[&with_on_top(overlay)], &[EDITOR]).is_empty());
}

/// An opaque always-on-top window of another app is masked like a toast;
/// a layered one too, since only click-through overlays are exempt.
#[test]
fn another_apps_window_on_top_is_masked_even_when_layered() {
    let mut player = window(TOAST, Rect::new(0, 0, 300, 300), "PiP", 50);
    player.layered = true;
    assert_eq!(
        foreign_rects(&[&with_on_top(player)], &[EDITOR]),
        vec![Rect::new(100, 100, 300, 300)]
    );
}

#[test]
fn a_window_below_the_step_window_is_not_masked() {
    let s = snapshot(EDITOR, Some(EDITOR), desktop());
    assert!(foreign_rects(&[&s], &[EDITOR]).is_empty(), "browser below");
}

#[test]
fn the_steps_own_menus_dropdowns_and_owned_windows_are_not_masked() {
    let menu = window(MENU, Rect::new(850, 600, 1100, 950), "#32768", 10);
    let mut dropdown = window(DIALOG, Rect::new(200, 200, 400, 400), "Drop", 10);
    dropdown.popup = true;
    let mut palette = window(9, Rect::new(500, 150, 700, 300), "Palette", 11);
    palette.root_owner = EDITOR;
    let mut windows = desktop();
    windows.splice(0..0, [menu, dropdown, palette]);
    let s = snapshot(EDITOR, Some(EDITOR), windows);
    assert!(foreign_rects(&[&s], &[EDITOR]).is_empty());
}

/// The user clicked the toast where it lies over the window: it is what
/// the step documents, so it stays visible.
#[test]
fn the_window_under_the_click_is_not_masked() {
    let mut s = with_on_top(toast());
    s.under_click = Some(TOAST);
    let step = step_windows(&s, Some((800, 600)));
    assert!(foreign_rects(&[&s], &step).is_empty());
}

#[test]
fn a_click_into_the_notification_center_keeps_it_unmasked() {
    let center = window(
        TOAST,
        Rect::new(1400, 0, 1920, 1032),
        "Windows.UI.Core.CoreWindow",
        50,
    );
    let mut s = with_on_top(center);
    s.under_click = Some(TOAST);
    let step = step_windows(&s, Some((1500, 300)));
    assert_eq!(step, vec![TOAST]);
    assert!(foreign_rects(&[&s], &step).is_empty());
}

/// A window between a dialog and its owner covers only the owner: masked
/// there, never over the dialog drawn above it.
#[test]
fn a_window_between_a_dialog_and_its_owner_is_masked_over_the_owner_only() {
    let mut dialog = window(DIALOG, Rect::new(300, 300, 600, 500), "#32770", 10);
    dialog.root_owner = EDITOR;
    let other = window(BROWSER, Rect::new(500, 400, 1000, 800), "Chrome", 20);
    let s = snapshot(
        DIALOG,
        Some(DIALOG),
        vec![dialog, other, window(EDITOR, EDITOR_RECT, "Notepad", 10)],
    );
    let step = step_windows(&s, Some((400, 400)));
    assert_eq!(step, vec![DIALOG, EDITOR]);
    // The owner's overlap, minus the dialog's corner drawn above `other`.
    assert_eq!(
        foreign_rects(&[&s], &step),
        Rect::new(500, 400, 900, 700).subtract(&Rect::new(300, 300, 600, 500))
    );
    let masked = foreign_rects(&[&s], &step);
    assert!(masked
        .iter()
        .all(|r| r.intersect(&Rect::new(300, 300, 600, 500)).is_none()));
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

/// The toast host is `WS_EX_NOREDIRECTIONBITMAP` without `WS_EX_LAYERED`
/// and draws a transparent margin around the toast, so it hides nothing.
#[test]
fn a_field_behind_a_window_without_a_redirection_bitmap_is_masked() {
    const TOPMOST: u32 = 0x0000_0008;
    const NO_REDIRECTION_BITMAP: u32 = 0x0020_0000;
    const LAYERED: u32 = 0x0008_0000;
    assert!(see_through(TOPMOST | NO_REDIRECTION_BITMAP));
    assert!(see_through(LAYERED));
    assert!(!see_through(TOPMOST));
    let mut host = cover();
    host.layered = see_through(TOPMOST | NO_REDIRECTION_BITMAP);
    assert_eq!(field_behind(host), vec![Rect::new(300, 300, 500, 330)]);
}

/// Any app's context menu is `#32768`; only the step window's own is its menu.
#[test]
fn a_menu_of_another_process_is_masked() {
    let theirs = window(MENU, Rect::new(850, 600, 1100, 950), "#32768", 99);
    assert_eq!(
        foreign_rects(&[&with_on_top(theirs)], &[EDITOR]),
        vec![Rect::new(850, 600, 900, 700)]
    );
    let ours = window(MENU, Rect::new(850, 600, 1100, 950), "#32768", 10);
    assert!(foreign_rects(&[&with_on_top(ours)], &[EDITOR]).is_empty());
}

/// explorer.exe runs both the file window and the taskbar's live previews of
/// other apps: a preview or a shell surface is no dropdown of the window.
#[test]
fn a_same_process_thumbnail_or_shell_popup_over_explorer_is_masked() {
    let explorer = window(EDITOR, EDITOR_RECT, "CabinetWClass", 30);
    for class in ["TaskListThumbnailWnd", "Windows.UI.Core.CoreWindow"] {
        let mut popup = window(MENU, Rect::new(700, 500, 1000, 800), class, 30);
        popup.popup = true;
        let s = snapshot(EDITOR, Some(EDITOR), vec![popup, explorer.clone()]);
        assert_eq!(
            foreign_rects(&[&s], &[EDITOR]),
            vec![Rect::new(700, 500, 900, 700)],
            "{class}"
        );
    }
}
