import { useCallback, useEffect } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LoginScreen } from "./components/LoginScreen";
import { RecorderScreen } from "./components/RecorderScreen";
import { SettingsPage } from "./components/SettingsPage";
import { useAuth } from "./hooks/useAuth";
import { useRecorder } from "./hooks/useRecorder";
import { useSSE } from "./hooks/useSSE";
import { runGeneration } from "./lib/tauri";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

const VERSION = "0.7.0";
const IS_DEV = import.meta.env.DEV;

const IDLE_SIZE = new LogicalSize(460, 380);
const COMPACT_SIZE = new LogicalSize(160, 44);

function App() {
  // If this window is the settings window, render settings page
  const isSettingsWindow = window.location.search.includes("page=settings");
  if (isSettingsWindow) {
    return <SettingsPage isDev={IS_DEV} />;
  }

  return <MainApp />;
}

function MainApp() {
  const auth = useAuth();
  const recorder = useRecorder();

  // SSE event handling
  useSSE({
    onStatus: (msg) => recorder.setStatusMessage(msg),
    onError: (msg) => recorder.setStatusMessage(msg),
  });

  // Window mode switching
  useEffect(() => {
    const appWindow = getCurrentWindow();
    if (recorder.status === "recording") {
      appWindow.setSize(COMPACT_SIZE);
      appWindow.setAlwaysOnTop(true);
      appWindow.setDecorations(false);
      appWindow.setResizable(false);
    } else {
      appWindow.setSize(IDLE_SIZE);
      appWindow.setAlwaysOnTop(false);
      appWindow.setDecorations(true);
      appWindow.setResizable(false);
    }
  }, [recorder.status]);

  const handleOpenSettings = useCallback(async () => {
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
  }, []);

  const handleStart = useCallback(async () => {
    await recorder.start();
  }, [recorder]);

  const handleStop = useCallback(async () => {
    await recorder.stop();
  }, [recorder]);

  const handleOpenFolder = useCallback(async () => {
    if (recorder.outputDir) {
      try {
        await revealItemInDir(recorder.outputDir);
      } catch (e) {
        console.error("Failed to open folder:", e);
      }
    }
  }, [recorder.outputDir]);

  const handleRetry = useCallback(async () => {
    if (recorder.outputDir) {
      recorder.setProcessing();
      try {
        await runGeneration(recorder.outputDir);
        recorder.setStatusMessage("");
      } catch {
        // error handled via SSE
      }
    }
  }, [recorder]);

  // Loading state
  if (auth.loading) {
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100vh", background: "#0a0a14" }}>
        <p style={{ fontSize: 13, color: "#9090b0" }}>...</p>
      </div>
    );
  }

  // Not logged in -> show login
  if (!auth.loggedIn) {
    return (
      <LoginScreen
        onLogin={auth.login}
        loading={auth.loading}
        error={auth.error}
        onOpenSettings={handleOpenSettings}
        version={VERSION}
      />
    );
  }

  // Logged in -> show recorder
  return (
    <RecorderScreen
      email={auth.email}
      status={recorder.status}
      statusMessage={recorder.statusMessage}
      error={recorder.error}
      outputDir={recorder.outputDir}
      onStart={handleStart}
      onStop={handleStop}
      onSignOut={auth.logout}
      onOpenSettings={handleOpenSettings}
      onOpenFolder={handleOpenFolder}
      onRetry={handleRetry}
      version={VERSION}
    />
  );
}

export default App;
