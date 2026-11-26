/// Relaunch the app.
///
/// macOS applies a newly granted Screen Recording or Accessibility permission
/// only to a fresh process, so the honest first-run flow is one deliberate
/// restart the user chooses rather than two they stumble into.
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    log::info!("Restarting to pick up newly granted permissions");
    // `restart` starts the new process before this one exits. Still holding
    // the single-instance lock, the new one would take itself for a second
    // launch, hand over to this one and quit -- and no app would come back.
    tauri_plugin_single_instance::destroy(&app);
    app.restart();
}

/// Show a window and give it focus.
pub fn bring_to_front(app: &tauri::AppHandle, label: &str) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.show();
        // On Windows, foreground-lock rules may turn this into a flashing
        // taskbar button rather than a raised window. Accepted: it still
        // points the user at the app.
        let _ = window.set_focus();
    }
}

/// The app was launched again while running: point the user at the copy they
/// already have instead of starting a second one -- which would bring its own
/// settings writes, keyring migration and, worst, a second recorder able to
/// record the same session in parallel.
pub fn focus_running_instance(app: &tauri::AppHandle) {
    let bar_visible = crate::recording_bar::is_visible(app);
    log::info!("Second launch; raising the running instance");
    bring_to_front(app, window_to_raise(bar_visible));
}

/// Closing the main window quits the app. Tauri only exits once every window
/// is gone, and the recording bar lives hidden for the app's whole life, so
/// without this the process kept running with nothing on screen -- and the
/// next launch handed over to it and showed nothing either. A recording never
/// closes the main window, only hides it.
pub fn quit_when_main_closes(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::Manager;
    if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
        log::info!("Main window closed; quitting");
        window.app_handle().exit(0);
    }
}

/// While recording, the main window is hidden behind the compact bar on
/// purpose; showing it would cover the app being recorded.
fn window_to_raise(bar_visible: bool) -> &'static str {
    if bar_visible {
        crate::recording_bar::LABEL
    } else {
        "main"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_launch_raises_the_bar_while_recording_and_the_main_window_otherwise() {
        assert_eq!(window_to_raise(true), crate::recording_bar::LABEL);
        assert_eq!(window_to_raise(false), "main");
    }
}
