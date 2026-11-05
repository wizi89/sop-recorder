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
    use super::super::window_area::{Rect, Snapshot, WindowInfo};
    use std::ffi::c_void;
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE};
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::UI::WindowsAndMessaging::*;

    pub fn snapshot(click: Option<(i32, i32)>) -> Snapshot {
        let mut windows: Vec<WindowInfo> = Vec::new();
        unsafe {
            // Top of the z-order first, which is the order D4's visibility
            // calculation reads it in.
            let _ = EnumWindows(Some(collect), LPARAM(&mut windows as *mut _ as isize));
            let foreground = GetForegroundWindow();
            let under_click =
                click.map(|(x, y)| GetAncestor(WindowFromPoint(POINT { x, y }), GA_ROOT));
            Snapshot {
                foreground: (!foreground.0.is_null()).then_some(foreground.0 as isize),
                under_click: under_click.filter(|h| !h.0.is_null()).map(|h| h.0 as isize),
                windows,
            }
        }
    }

    unsafe extern "system" fn collect(hwnd: HWND, out: LPARAM) -> BOOL {
        let out = &mut *(out.0 as *mut Vec<WindowInfo>);
        if let Some(info) = info(hwnd) {
            out.push(info);
        }
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
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut affinity: u32 = WDA_NONE.0;
        let _ = GetWindowDisplayAffinity(hwnd, &mut affinity);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut class = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut class).max(0) as usize;

        Some(WindowInfo {
            hwnd: hwnd.0 as isize,
            rect,
            class: String::from_utf16_lossy(&class[..len]),
            pid,
            popup: style & WS_POPUP.0 != 0,
            root_owner: GetAncestor(hwnd, GA_ROOTOWNER).0 as isize,
            cloaked: cloaked != 0,
            layered: ex_style & WS_EX_LAYERED.0 != 0,
            transparent: ex_style & WS_EX_TRANSPARENT.0 != 0,
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
                "{:>10x} {:?} {} pid {} popup {} cloaked {} layered {} excluded {} owner {:x}",
                w.hwnd,
                w.rect,
                w.class,
                w.pid,
                w.popup,
                w.cloaked,
                w.layered,
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
