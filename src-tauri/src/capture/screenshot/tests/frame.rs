//! `frame_step` on synthetic pixels and snapshots: what a step keeps, with no
//! display.

use super::*;
use crate::capture::window_area::{Snapshot, WindowInfo};

const EDITOR: isize = 1;
const BROWSER: isize = 2;
const DESKTOP: isize = 3;
const TOAST: isize = 4;
const ON_TOP: isize = 5;

const EDITOR_RECT: Rect = Rect::new(100, 100, 900, 700);
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

/// The editor in front, a browser behind it, the desktop; `top` windows sit
/// above all of them.
fn snapshot(top: Vec<WindowInfo>) -> Snapshot {
    let mut windows = top;
    windows.extend([
        window(EDITOR, EDITOR_RECT, "Notepad", 10),
        window(
            BROWSER,
            Rect::new(800, 50, 1800, 900),
            "Chrome_WidgetWin_1",
            20,
        ),
        window(DESKTOP, MONITOR, "Progman", 30),
    ]);
    Snapshot {
        foreground: Some(EDITOR),
        under_click: Some(EDITOR),
        windows,
        partial: false,
    }
}

fn read(before: Option<Snapshot>, after: Option<Snapshot>) -> PixelRead {
    PixelRead {
        img: blank(1920, 1080),
        screen: screen((0, 0), 1.0),
        before,
        after,
        pixels_at: std::time::Instant::now(),
    }
}

#[test]
fn monitor_mode_keeps_the_whole_monitor() {
    let r = read(Some(snapshot(vec![])), Some(snapshot(vec![])));
    let held = frame_step(r, 1, Some((300, 300)), true);
    assert_eq!(held.scope, CaptureScope::Monitor);
    assert_eq!((held.img.width(), held.img.height()), (1920, 1080));
    assert!(held.keep.is_empty());
    assert!(held.foreign.is_empty());
    assert!(held.geometry.is_some());
}

/// Off Windows there are no snapshots: the active window mode keeps the
/// monitor and marks the step.
#[test]
fn no_window_geometry_falls_back_to_the_monitor() {
    let held = frame_step(read(None, None), 1, Some((300, 300)), false);
    assert_eq!(held.scope, CaptureScope::MonitorFallback);
    assert_eq!((held.img.width(), held.img.height()), (1920, 1080));
    assert!(held.keep.is_empty());
    assert!(held.foreign.is_empty());
    assert!(held.geometry.is_none());
}

/// The crop is the editor, its pixels start at the editor's corner, and a
/// window over it in either snapshot is blacked out: one present only before
/// the read, one that appeared during it.
#[test]
fn the_active_window_is_cropped_and_foreign_windows_from_both_snapshots_masked() {
    let mut r = read(
        Some(snapshot(vec![window(
            ON_TOP,
            Rect::new(50, 50, 200, 200),
            "OnTop",
            50,
        )])),
        Some(snapshot(vec![window(
            TOAST,
            Rect::new(700, 500, 1000, 800),
            "Toast",
            40,
        )])),
    );
    let corner = Rgba([200, 0, 0, 255]);
    r.img.put_pixel(100, 100, corner);

    let held = frame_step(r, 1, Some((300, 300)), false);
    assert_eq!(held.scope, CaptureScope::Window);
    assert_eq!((held.img.width(), held.img.height()), (800, 600));
    assert_eq!(*held.img.get_pixel(0, 0), corner);
    assert_eq!(held.screen.origin, (100, 100));
    assert_eq!(held.keep, vec![EDITOR_RECT]);
    assert_eq!(
        held.foreign,
        vec![Rect::new(100, 100, 200, 200), Rect::new(700, 500, 900, 700)]
    );
    assert!(!held.geometry.unwrap().foreground_changed);
}

/// A window smaller than one captured pixel: the area is on the monitor but
/// covers no pixel, so the step keeps the monitor instead of an empty image.
#[test]
fn an_area_outside_the_captured_pixels_falls_back_to_the_monitor() {
    let tiny = Rect::new(101, 101, 102, 102);
    let mut s = snapshot(vec![]);
    s.windows[0].rect = tiny;
    let r = PixelRead {
        img: blank(1000, 1000),
        screen: screen((0, 0), 0.5),
        before: Some(s.clone()),
        after: Some(s),
        pixels_at: std::time::Instant::now(),
    };
    let held = frame_step(r, 1, Some((101, 101)), false);
    assert_eq!(held.scope, CaptureScope::MonitorFallback);
    assert_eq!((held.img.width(), held.img.height()), (1000, 1000));
    assert!(held.keep.is_empty());
    assert!(held.foreign.is_empty());
}
