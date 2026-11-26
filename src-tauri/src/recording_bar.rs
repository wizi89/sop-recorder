//! The recording bar: the small window that stands in for the main window
//! while a recording runs, and the one place that knows its label, where it
//! sits and how it stays out of the screenshots and out of the steps.

use tauri::Manager;

/// Label of the window that carries the compact recording bar. `main.tsx`
/// routes on the same string.
///
/// The bar is a window of its own rather than the main window resized, for a
/// reason that is entirely macOS's: whether a window may appear inside another
/// app's fullscreen Space is decided when the window is *created*, from the
/// application's activation policy at that instant. It cannot be changed
/// afterwards. Measured, on macOS 26:
///
///   level 25 or 101, regular app, existing window ......... not in the Space
///   collectionBehavior re-asserted after the fact ......... not in the Space
///   window hidden and re-shown after the fact ............. not in the Space
///   nonactivatingPanel style bit on a plain NSWindow ...... not in the Space
///   window created while the app is an accessory app ...... visible
///
/// So the bar is built once, at startup, inside a momentary dip to accessory
/// policy -- and the main window, created by Tauri before this runs, keeps the
/// ordinary behaviour a document window should have.
pub const LABEL: &str = "bar";

/// Gap between the bar and the edges of the work area, in logical pixels.
const MARGIN: f64 = 12.0;

/// Where the bar sits in a work area: its bottom-right corner, `MARGIN` in
/// from both edges. All in physical pixels; `scale` turns the margin into them.
pub fn corner(work_area: (i32, i32, u32, u32), bar: (u32, u32), scale: f64) -> (i32, i32) {
    let (x, y, width, height) = work_area;
    let margin = (MARGIN * scale).round() as i32;
    (
        x + width as i32 - bar.0 as i32 - margin,
        y + height as i32 - bar.1 as i32 - margin,
    )
}

fn bar(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Show the bar in the corner of the primary monitor's work area (the screen
/// minus the taskbar, or minus the menu bar and the Dock), and tell the input
/// hook where it landed.
///
/// The region is reported after the move, because only then is the position
/// final. It is in the logical points the input hook reports cursor positions
/// in. When the position is unknown the bar is shown where it is and no region
/// is reported, so a stale one cannot swallow real clicks.
pub fn show(app: &tauri::AppHandle) -> Result<(), String> {
    // Without the bar there is no way to stop a recording, so this is fatal
    // rather than a degraded mode worth limping along in.
    let bar = bar(app).ok_or("The recording bar window is missing")?;
    match place_in_corner(&bar) {
        Ok(region) => {
            log::info!("Recorder region reported as {:?}", region);
            crate::capture::input_hooks::set_excluded_region(Some(region));
        }
        Err(e) => {
            log::warn!("Could not anchor the recording bar: {e}");
            crate::capture::input_hooks::set_excluded_region(None);
        }
    }
    bar.show().map_err(|e| e.to_string())
}

/// Move the bar to its corner and return where it now is, in logical points.
fn place_in_corner(bar: &tauri::WebviewWindow) -> Result<(i32, i32, i32, i32), String> {
    let monitor = bar
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No primary monitor")?;
    let area = monitor.work_area();
    let scale = bar.scale_factor().map_err(|e| e.to_string())?;
    let size = bar.outer_size().map_err(|e| e.to_string())?;
    let (x, y) = corner(
        (
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        ),
        (size.width, size.height),
        scale,
    );
    bar.set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    let logical = |v: f64| (v / scale).round() as i32;
    Ok((
        logical(x as f64),
        logical(y as f64),
        logical(size.width as f64),
        logical(size.height as f64),
    ))
}

/// Hide the bar. The region is cleared first, so a stale one cannot swallow
/// real clicks in the next recording.
pub fn hide(app: &tauri::AppHandle) -> Result<(), String> {
    log::info!("Recorder region reported as None");
    crate::capture::input_hooks::set_excluded_region(None);
    match bar(app) {
        Some(bar) => bar.hide().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[tauri::command]
pub fn show_recording_bar(app: tauri::AppHandle) -> Result<(), String> {
    show(&app)
}

#[tauri::command]
pub fn hide_recording_bar(app: tauri::AppHandle) -> Result<(), String> {
    hide(&app)
}

/// The bar's native window handle, so the input hook can ignore clicks on it
/// (Windows only).
pub fn hwnd(app: &tauri::AppHandle) -> Option<isize> {
    #[cfg(windows)]
    {
        bar(app).and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        None
    }
}

/// Whether the bar is on screen, which means a recording runs.
pub fn is_visible(app: &tauri::AppHandle) -> bool {
    bar(app)
        .and_then(|bar| bar.is_visible().ok())
        .unwrap_or(false)
}

/// Exclude the bar from screen captures, or stop excluding it.
///
/// Targets the recording bar, which is the only window on screen while a
/// recording runs -- the main window is hidden for the duration.
pub fn hide_from_capture(app: &tauri::AppHandle, hide: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity;
        use windows::Win32::UI::WindowsAndMessaging::{WDA_EXCLUDEFROMCAPTURE, WDA_NONE};

        let window = bar(app).ok_or("Recording bar not found")?;

        // Get the native HWND
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let affinity = if hide {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };

        unsafe {
            SetWindowDisplayAffinity(HWND(hwnd.0 as *mut _), affinity)
                .map_err(|e| format!("SetWindowDisplayAffinity failed: {}", e))?;
        }

        log::info!(
            "Display affinity set to {}",
            if hide { "hidden" } else { "visible" }
        );
    }

    // macOS has a direct analogue of WDA_EXCLUDEFROMCAPTURE: an NSWindow whose
    // sharingType is None is skipped by CGWindowListCreateImage, which is the
    // call the screenshot path goes through. Without it the recorder's own
    // control bar ("stop recording") is composited into every screenshot,
    // sitting on top of the very thing the step is meant to document.
    #[cfg(target_os = "macos")]
    {
        let window = bar(app).ok_or("Recording bar not found")?;

        let ns_window = window.ns_window().map_err(|e| e.to_string())?;
        if ns_window.is_null() {
            return Err("Recording bar has no NSWindow".into());
        }

        // AppKit is main-thread-only and traps the process on violation
        // ("Must only be used from the main thread"). `start_recording` is an
        // async command, so this arrives on a tokio worker and touching the
        // NSWindow from here killed the app the moment a recording started.
        //
        // Posted rather than awaited: the caller needs no answer, and blocking
        // a worker on the main thread to set a flag buys nothing but a way to
        // deadlock. The window is excluded well within the frame before the
        // first screenshot.
        let ns_window = ns_window as usize;
        match objc2::MainThreadMarker::new() {
            Some(mtm) => apply_sharing_type(ns_window, hide, mtm),
            None => app
                .run_on_main_thread(move || {
                    // Sound by construction: this closure runs on the main
                    // thread, which is exactly what the marker asserts.
                    let mtm = objc2::MainThreadMarker::new().unwrap();
                    apply_sharing_type(ns_window, hide, mtm);
                })
                .map_err(|e| format!("Failed to hop to the main thread: {}", e))?,
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (app, hide);
        log::warn!("Display affinity is not supported on this platform");
    }

    Ok(())
}

/// Set the bar's NSWindow sharing type.
///
/// Takes the NSWindow as a `usize` because a raw pointer is not `Send` and this
/// has to cross a thread boundary to reach the main thread. The marker is the
/// proof that it arrived there.
#[cfg(target_os = "macos")]
fn apply_sharing_type(ns_window: usize, hide: bool, _mtm: objc2::MainThreadMarker) {
    use objc2_app_kit::{NSWindow, NSWindowSharingType};

    // The pointer belongs to the live bar window; this borrow ends here.
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    window.setSharingType(if hide {
        NSWindowSharingType::None
    } else {
        NSWindowSharingType::ReadOnly
    });

    log::info!(
        "Display affinity set to {}",
        if hide { "hidden" } else { "visible" }
    );
}

/// Move the recording bar to the corner of the monitor the user clicked on, so
/// it stays in sight while they work on another monitor. Same corner and
/// margin as `show`.
///
/// Windows only: there the hook reports clicks in physical pixels, the space
/// monitors are given in, and the bar is excluded from capture by its window
/// handle, so no region needs reporting again.
/// ponytail: macOS gets this when someone records there on two monitors; it
/// needs the click converted from points and the region reported again.
#[cfg(windows)]
pub fn follow_click(app: &tauri::AppHandle, click: (i32, i32)) {
    let Some(bar) = bar(app) else {
        return;
    };
    let Ok(monitors) = bar.available_monitors() else {
        return;
    };
    let bounds: Vec<_> = monitors
        .iter()
        .map(|m| {
            let (p, s) = (m.position(), m.size());
            (p.x, p.y, s.width, s.height)
        })
        .collect();
    let Some(target) =
        crate::capture::screenshot::monitor_containing(Some(click), &bounds).map(|i| &monitors[i])
    else {
        return;
    };
    let current = bar.current_monitor().ok().flatten();
    if current.as_ref().map(|m| *m.position()) == Some(*target.position()) {
        return;
    }
    let (Ok(size), Ok(scale)) = (bar.outer_size(), bar.scale_factor()) else {
        return;
    };
    // The bar takes the target monitor's scale once it is there.
    let ratio = target.scale_factor() / scale;
    let resized = (
        (size.width as f64 * ratio).round() as u32,
        (size.height as f64 * ratio).round() as u32,
    );
    let area = target.work_area();
    let (x, y) = corner(
        (
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        ),
        resized,
        target.scale_factor(),
    );
    match bar.set_position(tauri::PhysicalPosition::new(x, y)) {
        Ok(()) => log::info!("Recording bar moved to the clicked monitor at ({x}, {y})"),
        Err(e) => log::warn!("Recording bar could not follow the click: {e}"),
    }
}

/// The collection-behavior flags that make a window an overlay.
///
/// CanJoinAllSpaces    -- show on every Space, fullscreen ones included.
/// FullScreenAuxiliary -- may share a Space with a fullscreen window rather
///                        than being suppressed by it.
/// Stationary          -- do not slide during the Space-switch animation.
///
/// Merged into whatever the window already carries rather than assigned, so
/// nothing Tauri set for its own purposes is silently dropped.
#[cfg(target_os = "macos")]
fn overlay_behavior(
    current: objc2_app_kit::NSWindowCollectionBehavior,
) -> objc2_app_kit::NSWindowCollectionBehavior {
    use objc2_app_kit::NSWindowCollectionBehavior as B;
    current.union(B::CanJoinAllSpaces | B::FullScreenAuxiliary | B::Stationary)
}

/// Build the recording bar's window during a momentary dip to accessory policy.
///
/// Runs from `setup`, which Tauri calls on the main thread -- the only thread
/// AppKit tolerates. The policy is restored before returning, so the app keeps
/// its Dock icon and menu bar for its whole life; the dip is measured in
/// microseconds and is not observable.
pub fn create(app: &tauri::AppHandle) -> Result<(), String> {
    let builder =
        tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("index.html".into()))
            .title("CogniClone Recording")
            .inner_size(240.0, 34.0)
            .decorations(false)
            .resizable(false)
            .shadow(false)
            // `tauri.conf.json` sets this for the main window and it does not reach a
            // window built here. Without it the webview starts light and shows a white
            // flash in the instant the bar appears over whatever is being recorded.
            .theme(Some(tauri::Theme::Dark))
            // The bar is never the key window -- it is an overlay the user reaches
            // over whatever they are actually working in. Without this macOS spends
            // the first click activating the window and only the second one reaches
            // Stop, which reads as the button being broken. `tauri.conf.json` sets
            // this for the main window, and that setting does not reach a window
            // built here.
            .accept_first_mouse(true)
            // Shown when a recording starts, not before.
            .visible(false)
            // Windows keeps its taskbar button: before the bar was its own window the
            // main window shrank into this role and stayed in the taskbar, and that is
            // the only way back to a bar the user has lost behind something. macOS has
            // no equivalent -- the Dock shows the app, not the window -- so the flag
            // would only add a stray entry there.
            .skip_taskbar(cfg!(target_os = "macos"));

    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{
            NSApplication, NSApplicationActivationPolicy, NSPopUpMenuWindowLevel, NSWindow,
        };

        let mtm = objc2::MainThreadMarker::new()
            .ok_or("recording_bar::create must run on the main thread")?;
        let ns_app = NSApplication::sharedApplication(mtm);

        ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let built = builder.build().map_err(|e| e.to_string());
        ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let window = built?;

        let ns_window = window.ns_window().map_err(|e| e.to_string())?;
        if ns_window.is_null() {
            return Err("Recording bar has no NSWindow".into());
        }
        let ns_window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };

        ns_window.setCollectionBehavior(overlay_behavior(ns_window.collectionBehavior()));
        // Above the fullscreen chrome apps draw for themselves: Chromium puts
        // its fullscreen menu bar at layer 26, one above NSStatusWindowLevel,
        // so status level would leave the bar underneath Brave's own toolbar.
        ns_window.setLevel(NSPopUpMenuWindowLevel);

        // Read back rather than log what we asked for: AppKit drops
        // collection-behavior flags that conflict, silently.
        log::info!(
            "Recording bar built: behavior={:#010x} level={}",
            ns_window.collectionBehavior().0,
            ns_window.level()
        );
    }

    // Windows has no Spaces, so a topmost window is already on whatever the
    // user is looking at, fullscreen included. always_on_top is the whole fix
    // there, and it is what the main window used to do in this role.
    #[cfg(not(target_os = "macos"))]
    {
        builder
            .always_on_top(true)
            .build()
            .map_err(|e| e.to_string())?;
        log::info!("Recording bar built (always on top)");
    }

    Ok(())
}

#[cfg(test)]
mod tests;
