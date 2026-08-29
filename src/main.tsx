import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { RecordingBar } from "./components/RecordingBar";
import "./styles.css";

// Two windows share this bundle. The recording bar is a window of its own for
// a macOS reason documented at `BAR_LABEL` in commands/window.rs, and the label
// is what tells the two apart.
const isBar = getCurrentWindow().label === "bar";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isBar ? <RecordingBar /> : <App />}</React.StrictMode>,
);
