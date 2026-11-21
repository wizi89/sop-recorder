import { useCallback, useEffect } from "react";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getWorkArea, setRecorderRegion } from "../lib/tauri";
import type { RecorderStatus } from "./useRecorder";

// Must match the label `create_recording_bar` builds the window under.
const BAR_LABEL = "bar";

/** The recording bar in place of the main window while a recording runs. */
export function useCompactMode(status: RecorderStatus) {
  /// Show the recording bar, anchor it to the corner of the work area, hide
  /// the main window, and tell the capture side where the bar landed.
  ///
  /// Awaited before the recording starts rather than reacted to afterwards.
  /// The input hook goes live inside `start_recording`, so anchoring after it
  /// left a window in which a click on the bar was still captured as a step --
  /// the exact defect the region exists to prevent.
  const enterCompactMode = useCallback(async () => {
    const appWindow = getCurrentWindow();
    const bar = await WebviewWindow.getByLabel(BAR_LABEL);
    if (!bar) {
      // Without the bar there is no way to stop a recording, so this is fatal
      // rather than a degraded mode worth limping along in.
      throw new Error("The recording bar window is missing");
    }

    const MARGIN = 12;
    try {
      const [area, scale, outerSize] = await Promise.all([
        getWorkArea(),
        bar.scaleFactor(),
        bar.outerSize(),
      ]);
      const margin = Math.round(MARGIN * scale);
      const x = area.x + area.width - outerSize.width - margin;
      const y = area.y + area.height - outerSize.height - margin;
      await bar.setPosition(new PhysicalPosition(x, y));
      // In the logical points the input hook reports cursor positions in.
      await setRecorderRegion([
        Math.round(x / scale),
        Math.round(y / scale),
        Math.round(outerSize.width / scale),
        Math.round(outerSize.height / scale),
      ]);
    } catch (e) {
      // Position unknown: leave the bar where it is and report no region, so a
      // stale one cannot swallow real clicks.
      console.warn("Could not anchor the recording bar:", e);
      await setRecorderRegion(null);
    }

    await bar.show();
    // Hidden, not closed: this window keeps running the recording and owns
    // every decision the bar's buttons ask for.
    await appWindow.hide();
  }, []);

  const exitCompactMode = useCallback(async () => {
    const appWindow = getCurrentWindow();
    await setRecorderRegion(null);
    const bar = await WebviewWindow.getByLabel(BAR_LABEL);
    await bar?.hide();
    await appWindow.show();
    await appWindow.setFocus();
  }, []);

  // Restores the full window when a recording ends. Entering compact mode is
  // done by the caller before starting, not here, so the bar is already in
  // place and its region already reported when the input hook goes live.
  useEffect(() => {
    if (status !== "recording") {
      void exitCompactMode();
    }
  }, [status, exitCompactMode]);

  return { enterCompactMode, exitCompactMode };
}
