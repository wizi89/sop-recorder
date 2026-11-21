import { useEffect } from "react";
import { TauriEvent } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { setErrorReportPhase } from "../lib/tauri";
import { safeUnlisten } from "../lib/safeUnlisten";
import type { Phase } from "../lib/errorPhase";

/** Publishes the main window's phase for error reports (D3), and again on focus. */
export function usePublishReportPhase(phase: Phase) {
  useEffect(() => {
    void setErrorReportPhase(phase);
  }, [phase]);

  // The settings window shares one phase with this one but has its own module
  // state, so when it closes nothing here knows the phase needs restoring.
  // Regaining focus is that signal.
  useEffect(() => {
    let cancelled = false;
    let stop: (() => void) | null = null;
    const off = () => {
      // Tauri's unlisten rejects asynchronously, so a try/catch here cannot
      // see the throw; it reaches the global handler as a report.
      safeUnlisten(stop);
      stop = null;
    };
    // WINDOW_FOCUS rather than onFocusChanged: its combined unlisten drops the
    // promises of the two listeners it wraps (see the same choice in App.tsx).
    void getCurrentWindow()
      .listen(TauriEvent.WINDOW_FOCUS, () => {
        void setErrorReportPhase(phase, true);
      })
      .then((fn) => {
        stop = fn;
        if (cancelled) off();
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      off();
    };
  }, [phase]);
}
