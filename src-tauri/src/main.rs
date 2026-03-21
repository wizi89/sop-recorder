// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Set DPI awareness before any window creation (Windows 8.1+)
    #[cfg(windows)]
    {
        unsafe {
            use windows::Win32::UI::HiDpi::SetProcessDpiAwareness;
            use windows::Win32::UI::HiDpi::PROCESS_PER_MONITOR_DPI_AWARE;
            let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
        }
    }

    sop_recorder_lib::run()
}
