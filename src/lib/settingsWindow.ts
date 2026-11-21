import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Open the settings window, or focus it if it is already open. */
export async function openSettingsWindow() {
  // Check if settings window already exists
  const existing = await WebviewWindow.getByLabel("settings");
  if (existing) {
    await existing.setFocus();
    return;
  }

  new WebviewWindow("settings", {
    url: "index.html?page=settings",
    title: "Einstellungen",
    width: 420,
    height: 480,
    resizable: false,
    center: true,
    decorations: true,
    theme: "dark",
  });
}
