import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { RecordingBar } from "./components/RecordingBar";
import { createErrorReport, errorReportPhase } from "./lib/tauri";
import "./styles.css";

// Two windows share this bundle. The recording bar is a window of its own for
// a macOS reason documented at `BAR_LABEL` in commands/window.rs, and the label
// is what tells the two apart.
const isBar = getCurrentWindow().label === "bar";

// Errors the boundary cannot see: a throw outside React's render path, and a
// promise nobody awaited. Both leave the app running but broken, and both used
// to reach nothing but the developer console (design D6). Only in the main
// window -- the bar reports nothing of its own, by design.
if (!isBar) {
  // One dialog per distinct failure, not one per occurrence. A broken effect
  // fires on every mount, and StrictMode mounts twice, so an unguarded handler
  // turns one fault into a queue of identical dialogs -- which is exactly what
  // happened on 2026-09-04, when a listener teardown threw twice within the
  // same millisecond and filed two reports about itself. Grouping still
  // happens server-side; this is about not burying the user in dialogs.
  const DEDUP_WINDOW_MS = 30_000;
  const lastReported = new Map<string, number>();

  const reportUiError = (message: string) => {
    const now = Date.now();
    const previous = lastReported.get(message);
    if (previous !== undefined && now - previous < DEDUP_WINDOW_MS) return;
    for (const [seen, at] of lastReported) {
      if (now - at >= DEDUP_WINDOW_MS) lastReported.delete(seen);
    }
    lastReported.set(message, now);
    void createErrorReport("ui_error", errorReportPhase(), message).catch(() => {});
  };

  window.addEventListener("error", (event) => {
    const message = event.error instanceof Error ? event.error.message : event.message;
    reportUiError(message);
  });
  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    const message = reason instanceof Error ? reason.message : String(reason);
    reportUiError(message);
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isBar ? (
      <RecordingBar />
    ) : (
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    )}
  </React.StrictMode>,
);
