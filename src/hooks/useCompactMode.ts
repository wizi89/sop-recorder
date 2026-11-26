import { useCallback, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { hideRecordingBar, showRecordingBar } from "../lib/tauri";
import type { RecorderStatus } from "./useRecorder";

/** The recording bar in place of the main window while a recording runs. */
export function useCompactMode(status: RecorderStatus) {
  /// Show the recording bar in its corner, then hide the main window.
  ///
  /// Awaited before the recording starts rather than reacted to afterwards.
  /// The input hook goes live inside `start_recording`, so anchoring after it
  /// left a window in which a click on the bar was still captured as a step --
  /// the exact defect the bar's reported region exists to prevent.
  const enterCompactMode = useCallback(async () => {
    await showRecordingBar();
    // Hidden, not closed: this window keeps running the recording and owns
    // every decision the bar's buttons ask for.
    await getCurrentWindow().hide();
  }, []);

  const exitCompactMode = useCallback(async () => {
    const appWindow = getCurrentWindow();
    await hideRecordingBar();
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
