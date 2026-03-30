use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Return the primary monitor's work area (screen minus taskbar).
#[tauri::command]
pub fn get_work_area() -> Result<WorkArea, String> {
    #[cfg(windows)]
    {
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

    #[cfg(not(windows))]
    {
        Err("get_work_area is only supported on Windows".into())
    }
}

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
