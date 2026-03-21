use tauri::Manager;

/// Toggle the SetWindowDisplayAffinity to hide/show the recorder from screenshots.
#[tauri::command]
pub fn set_display_affinity(app: tauri::AppHandle, hide: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity;
        use windows::Win32::UI::WindowsAndMessaging::{WDA_EXCLUDEFROMCAPTURE, WDA_NONE};
        use windows::Win32::Foundation::HWND;

        let window = app
            .get_webview_window("main")
            .ok_or("Main window not found")?;

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

    #[cfg(not(windows))]
    {
        let _ = (app, hide);
        log::warn!("Display affinity is only supported on Windows");
    }

    Ok(())
}
