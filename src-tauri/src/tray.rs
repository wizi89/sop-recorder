use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    App, Emitter, Manager,
};

pub fn create_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItemBuilder::with_id("show", "Anzeigen").build(app)?;
    let hide = MenuItemBuilder::with_id("hide", "Ausblenden").build(app)?;
    let start = MenuItemBuilder::with_id("start_recording", "Aufnahme starten").build(app)?;
    let stop = MenuItemBuilder::with_id("stop_recording", "Aufnahme stoppen").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "Einstellungen").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Beenden").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&hide)
        .separator()
        .item(&start)
        .item(&stop)
        .separator()
        .item(&settings)
        .separator()
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            tauri::image::Image::new_owned(vec![0; 32 * 32 * 4], 32, 32)
        }))
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "hide" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.hide();
                    }
                }
                "start_recording" => {
                    let _ = app.emit("tray:start_recording", ());
                }
                "stop_recording" => {
                    let _ = app.emit("tray:stop_recording", ());
                }
                "settings" => {
                    let _ = app.emit("tray:settings", ());
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
