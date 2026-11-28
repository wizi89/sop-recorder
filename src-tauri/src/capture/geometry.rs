//! The geometry snapshot taken before and after each step's pixel read
//! (privacy-capture D3). Windows only; elsewhere there is no snapshot and the
//! step is captured as the monitor.

use super::window_area::Snapshot;

#[cfg(windows)]
pub fn snapshot(click: Option<(i32, i32)>) -> Option<Snapshot> {
    Some(windows_impl::snapshot(click))
}

#[cfg(not(windows))]
pub fn snapshot(_click: Option<(i32, i32)>) -> Option<Snapshot> {
    None
}

#[cfg(windows)]
mod windows_impl {
    use super::super::window_area::{see_through, Rect, Snapshot, WindowInfo};
    use std::ffi::c_void;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        SetLastError, BOOL, HWND, LPARAM, POINT, RECT, TRUE, WIN32_ERROR,
    };
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::UI::WindowsAndMessaging::*;

    pub fn snapshot(click: Option<(i32, i32)>) -> Snapshot {
        unsafe {
            // Top of the z-order first, which is the order D4's visibility
            // calculation reads it in.
            let (list, partial) = top_level_windows();
            let windows: Vec<WindowInfo> = list.into_iter().filter_map(|hwnd| info(hwnd)).collect();
            let foreground = GetForegroundWindow();
            let under_click =
                click.map(|(x, y)| GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT));
            Snapshot {
                foreground: (!foreground.0.is_null()).then_some(foreground.0 as isize),
                under_click: under_click.filter(|h| !h.0.is_null()).map(|h| h.0 as isize),
                windows,
                partial,
            }
        }
    }

    /// Every top-level window, top of the z-order first.
    ///
    /// Not `EnumWindows`: since Windows 8 it leaves out immersive shell
    /// windows, and toasts, the notification center and quick settings are
    /// such windows, drawn over every app and present in the pixels (see
    /// FINDINGS). Walking the list with `FindWindowExW` returns them too.
    /// The walk breaks off when the window it continues from closes, so it is
    /// retried, and `EnumWindows` is the last resort; `true` marks that list
    /// as partial.
    ///
    /// A finished walk is not atomic either: a window that moves in the
    /// z-order mid-walk can be skipped. So one `EnumWindows` follows it, and a
    /// window it holds that the snapshot would keep but the walk lacks marks
    /// the list partial. It cannot catch a skipped immersive shell window,
    /// which `EnumWindows` leaves out too.
    unsafe fn top_level_windows() -> (Vec<HWND>, bool) {
        for _ in 0..3 {
            if let Some(list) = walk() {
                let missed = enum_windows()
                    .into_iter()
                    .filter(|h| !list.contains(h) && info(*h).is_some())
                    .count();
                if missed > 0 {
                    log::warn!("Window walk missed {missed} window(s); list marked partial");
                }
                return (list, missed > 0);
            }
        }
        log::warn!("Window walk broke off three times; using EnumWindows");
        (enum_windows(), true)
    }

    unsafe fn enum_windows() -> Vec<HWND> {
        let mut list: Vec<HWND> = Vec::new();
        // An empty list is flagged by the password check; the error is only
        // for the log.
        if let Err(e) = EnumWindows(Some(collect), LPARAM(&mut list as *mut _ as isize)) {
            log::warn!("EnumWindows failed: {e}");
        }
        list
    }

    /// One `FindWindowExW` walk; `None` when it broke off before the end.
    unsafe fn walk() -> Option<Vec<HWND>> {
        let mut list = Vec::new();
        let mut prev = HWND::default();
        // A guard against a z-order that changes into a cycle mid-walk.
        while list.len() < 50_000 {
            SetLastError(WIN32_ERROR(0));
            match FindWindowExW(HWND::default(), prev, PCWSTR::null(), PCWSTR::null()) {
                Ok(hwnd) => {
                    list.push(hwnd);
                    prev = hwnd;
                }
                // The end of the list sets no error.
                Err(e) if e.code().is_ok() => return Some(list),
                Err(_) => return None,
            }
        }
        None
    }

    unsafe extern "system" fn collect(hwnd: HWND, out: LPARAM) -> BOOL {
        let out = &mut *(out.0 as *mut Vec<HWND>);
        out.push(hwnd);
        TRUE
    }

    unsafe fn info(hwnd: HWND) -> Option<WindowInfo> {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        // Extended frame bounds exclude the invisible resize border, which
        // would otherwise widen every crop by a few pixels of whatever is
        // behind the window.
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            GetWindowRect(hwnd, &mut r).ok()?;
        }
        let rect = Rect::new(r.left, r.top, r.right, r.bottom);
        if rect.is_empty() {
            return None;
        }

        let mut cloaked: u32 = 0;
        let cloaked_read = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut affinity: u32 = WDA_NONE.0;
        let affinity_read = GetWindowDisplayAffinity(hwnd, &mut affinity);
        // A window whose cloaked state or capture exclusion cannot be read
        // may not be drawn, so it must not hide the windows below it from
        // the password check. It is still checked itself.
        let unsure = cloaked_read.is_err() || affinity_read.is_err();
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut class = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class).max(0) as usize;

        let click_through = ex_style & WS_EX_TRANSPARENT.0 != 0;
        Some(WindowInfo {
            hwnd: hwnd.0 as isize,
            rect,
            class: String::from_utf16_lossy(&class[..len]),
            pid,
            popup: style & WS_POPUP.0 != 0,
            root_owner: GetAncestor(hwnd, GA_ROOTOWNER).0 as isize,
            cloaked: cloaked != 0,
            layered: see_through(ex_style),
            transparent: unsure || click_through,
            click_through,
            excluded: affinity != WDA_NONE.0,
        })
    }
}

#[cfg(all(test, windows))]
mod tests {
    /// Reads the real desktop, so it needs one: `cargo test --lib live_snapshot
    /// -- --ignored --nocapture` and check the printout against the screen.
    #[test]
    #[ignore]
    fn live_snapshot() {
        let s = super::snapshot(None).unwrap();
        for w in &s.windows {
            println!(
                "{:>10x} {:?} {} pid {} popup {} cloaked {} layered {} click-through {} excluded {} owner {:x}",
                w.hwnd,
                w.rect,
                w.class,
                w.pid,
                w.popup,
                w.cloaked,
                w.layered,
                w.click_through,
                w.excluded,
                w.root_owner
            );
        }
        println!("foreground {:x?}", s.foreground);
        let monitor = super::super::window_area::Rect::new(0, 0, 3840, 2160);
        println!(
            "area {:?}",
            super::super::window_area::capture_area(&s, None, monitor)
        );
        assert!(
            s.foreground.is_some_and(|fg| s.get(fg).is_some()),
            "foreground not in the list"
        );
    }
}
