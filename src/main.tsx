import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { RecordingBar } from "./components/RecordingBar";
import { createErrorReport } from "./lib/tauri";
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
  window.addEventListener("error", (event) => {
    const message = event.error instanceof Error ? event.error.message : event.message;
    void createErrorReport("ui_error", "unknown", message).catch(() => {});
  });
  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    const message = reason instanceof Error ? reason.message : String(reason);
    void createErrorReport("ui_error", "unknown", message).catch(() => {});
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
