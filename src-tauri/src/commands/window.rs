use serde::Serialize;

#[derive(Serialize)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Return the primary monitor's work area (screen minus taskbar).
#[tauri::command]
pub fn get_work_area(app: tauri::AppHandle) -> Result<WorkArea, String> {
    #[cfg(windows)]
    {
        let _ = app;
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};

        let mut rect = RECT::default();
        unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut rect as *mut RECT as *mut _),
                Default::default(),
            )
            .map_err(|e| format!("SystemParametersInfoW failed: {}", e))?;
        }

        Ok(WorkArea {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        })
    }

    // macOS: NSScreen's visibleFrame is the screen minus the menu bar and the
    // Dock, which is what SPI_GETWORKAREA means on Windows. Without this the
    // command returned Err on macOS, the caller's `.catch` kept the window
    // where it was, and the recording bar stayed wherever the main window had
    // been -- centred, on top of the thing being recorded.
    #[cfg(target_os = "macos")]
    {
        // AppKit is main-thread-only. A Tauri command may run on either thread,
        // so take the marker when we already hold the main thread and hop when
        // we do not -- hopping unconditionally would deadlock in the first case.
        match objc2::MainThreadMarker::new() {
            Some(mtm) => main_thread_work_area(mtm),
            None => {
                let (tx, rx) = std::sync::mpsc::channel();
                app.run_on_main_thread(move || {
                    // `unwrap` is sound: this closure runs on the main thread
                    // by construction, which is what the marker asserts.
                    let mtm = objc2::MainThreadMarker::new().unwrap();
                    let _ = tx.send(main_thread_work_area(mtm));
                })
                .map_err(|e| format!("Failed to hop to the main thread: {}", e))?;
                rx.recv()
                    .map_err(|e| format!("Main thread did not answer: {}", e))?
            }
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = app;
        Err("get_work_area is not supported on this platform".into())
    }
}

/// Exclude the recorder's window from screen captures, or stop excluding it.
///
/// Takes the NSWindow as a `usize` because a raw pointer is not `Send` and this
/// has to cross a thread boundary to reach the main thread. The marker is the
/// proof that it arrived there.
#[cfg(target_os = "macos")]
fn apply_sharing_type(ns_window: usize, hide: bool, _mtm: objc2::MainThreadMarker) {
    use objc2_app_kit::{NSWindow, NSWindowSharingType};

    // The pointer belongs to the live main window; this borrow ends here.
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

/// The primary screen's usable area, in top-left-origin physical pixels.
///
/// AppKit works in bottom-left-origin points; the caller positions windows in
/// top-left-origin physical pixels, as Windows reports them. Both conversions
/// happen here so no caller has to know which convention it is holding.
#[cfg(target_os = "macos")]
fn main_thread_work_area(mtm: objc2::MainThreadMarker) -> Result<WorkArea, String> {
    use objc2_app_kit::NSScreen;

    let screen = NSScreen::mainScreen(mtm).ok_or("No main screen")?;
    let full = screen.frame();
    let visible = screen.visibleFrame();
    let scale = screen.backingScaleFactor();

    // Flip the origin: AppKit measures y up from the bottom of the screen.
    let top = full.size.height - (visible.origin.y + visible.size.height);

    Ok(WorkArea {
        x: (visible.origin.x * scale).round() as i32,
        y: (top * scale).round() as i32,
        width: (visible.size.width * scale).round() as i32,
        height: (visible.size.height * scale).round() as i32,
    })
}

/// Toggle the SetWindowDisplayAffinity to hide/show the recorder from screenshots.
///
/// Targets the recording bar, which is the only window on screen while a
/// recording runs -- the main window is hidden for the duration.
#[tauri::command]
pub fn set_display_affinity(app: tauri::AppHandle, hide: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use tauri::Manager;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity;
        use windows::Win32::UI::WindowsAndMessaging::{WDA_EXCLUDEFROMCAPTURE, WDA_NONE};

        let window = app
            .get_webview_window(BAR_LABEL)
            .ok_or("Recording bar not found")?;

        // Get the native HWND
        let hwnd = window.hwnd().map_err(|e| e.to_string())?;
        let affinity = if hide { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };

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
        use tauri::Manager;

        let window = app
            .get_webview_window(BAR_LABEL)
            .ok_or("Recording bar not found")?;

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

/// Tell the input hook where the recorder's compact bar is, so clicks on it are
/// not captured as steps of the process being recorded.
///
/// Reported by the frontend because that is what positions the window, and the
/// position is only final once it has. Coordinates are logical points with the
/// origin at the top-left of the primary display -- the space the input hook
/// reports cursor positions in. `None` means the bar is not on screen.
#[tauri::command]
pub fn set_recorder_region(region: Option<(i32, i32, i32, i32)>) {
    log::info!("Recorder region reported as {:?}", region);
    crate::capture::input_hooks::set_excluded_region(region);
}

/// Relaunch the app.
///
/// macOS applies a newly granted Screen Recording or Accessibility permission
/// only to a fresh process, so the honest first-run flow is one deliberate
/// restart the user chooses rather than two they stumble into.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    log::info!("Restarting to pick up newly granted permissions");
    app.restart();
}

/// Label of the window that carries the compact recording bar.
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
pub const BAR_LABEL: &str = "bar";

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
pub fn create_recording_bar(app: &tauri::AppHandle) -> Result<(), String> {
    let builder = tauri::WebviewWindowBuilder::new(
        app,
        BAR_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
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
            .ok_or("create_recording_bar must run on the main thread")?;
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

#[cfg(all(test, target_os = "macos"))]
mod overlay_tests {
    use super::overlay_behavior;
    use objc2_app_kit::NSWindowCollectionBehavior as B;

    #[test]
    fn the_three_overlay_flags_are_added() {
        let got = overlay_behavior(B::empty());
        assert!(got.contains(B::CanJoinAllSpaces));
        assert!(got.contains(B::FullScreenAuxiliary));
        assert!(got.contains(B::Stationary));
    }

    #[test]
    fn flags_the_window_already_had_survive() {
        let got = overlay_behavior(B::FullScreenPrimary);
        assert!(got.contains(B::FullScreenPrimary));
        assert!(got.contains(B::CanJoinAllSpaces));
    }
}
