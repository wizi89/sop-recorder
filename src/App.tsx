import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LoginScreen } from "./components/LoginScreen";
import { RecorderScreen } from "./components/RecorderScreen";
import { SettingsPage } from "./components/SettingsPage";
import { useAuth } from "./hooks/useAuth";
import { useRecorder } from "./hooks/useRecorder";
import { useSSE } from "./hooks/useSSE";
import { useUpdater } from "./hooks/useUpdater";
import { useTranslation } from "./hooks/useTranslation";
import { useQuota } from "./hooks/useQuota";
import {
  getWorkArea,
  getSettings,
  deleteLastScreenshot,
  getMicrophonePermissionState,
  type MicPermissionState,
} from "./lib/tauri";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
const IS_DEV = import.meta.env.DEV;

const IDLE_SIZE = new LogicalSize(460, 440);
const COMPACT_SIZE = new LogicalSize(240, 34);

function App() {
  // If this window is the settings window, render settings page
  const isSettingsWindow = window.location.search.includes("page=settings");
  if (isSettingsWindow) {
    return <SettingsPage isDev={IS_DEV} />;
  }

  return <MainApp />;
}

function MainApp() {
  const [version, setVersion] = useState("");
  const [skipPiiCheck, setSkipPiiCheck] = useState(false);
  const [micPermission, setMicPermission] = useState<MicPermissionState>("unknown");
  const { t } = useTranslation();
  const auth = useAuth();
  const recorder = useRecorder();
  const updater = useUpdater();
  // Quota hook is gated on login: only fetches once the user is authenticated.
  const quotaHook = useQuota(auth.loggedIn);

  // Keep the latest refresh() in a ref so effects that trigger a refresh
  // do not have to depend on the whole `quotaHook` object -- that object
  // gets a new identity on every render of useQuota, so including it in
  // a useEffect dep array causes an infinite re-render loop (refresh ->
  // setLoading -> re-render -> new object -> effect fires again...).
  const refreshQuotaRef = useRef(quotaHook.refresh);
  useEffect(() => {
    refreshQuotaRef.current = quotaHook.refresh;
  }, [quotaHook.refresh]);

  const loadSettings = useCallback(() => {
    getSettings()
      .then((s) => setSkipPiiCheck(s.skip_pii_check))
      .catch(() => {});
  }, []);

  useEffect(() => {
    getVersion().then(setVersion);
    loadSettings();
    // Query microphone permission state on launch so we can surface a
    // warning chip before the user attempts to record.
    getMicrophonePermissionState().then(setMicPermission);
  }, [loadSettings]);

  // Reload settings + quota when main window gains focus (e.g. after settings
  // window closes, or after the admin tops up the user's quota externally).
  // NOTE: must NOT depend on `quotaHook` -- it changes identity every render.
  useEffect(() => {
    const appWindow = getCurrentWindow();
    const unlisten = appWindow.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        loadSettings();
        if (auth.loggedIn) {
          void refreshQuotaRef.current();
        }
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, [loadSettings, auth.loggedIn]);

  // SSE event handling
  useSSE({
    onStatus: (msg) => recorder.setStatusMessage(msg),
    onError: (msg) => recorder.setError(msg),
    onPiiBlocked: (findings) => recorder.setPiiBlocked(findings),
  });

  // Refresh quota after every generation terminal transition (success, pii,
  // rate limit, or other error). Must only fire on TRANSITIONS into a
  // terminal state, not while still in one -- which is why `quotaHook` is
  // NOT a dep here (its identity changes every render and would cause an
  // infinite refresh loop once the UI lands in a terminal status).
  useEffect(() => {
    if (
      auth.loggedIn &&
      (recorder.status === "done" ||
        recorder.status === "rate_limited" ||
        recorder.status === "pii_blocked" ||
        recorder.status === "error")
    ) {
      void refreshQuotaRef.current();
    }
  }, [recorder.status, auth.loggedIn]);

  // Window mode switching
  useEffect(() => {
    const appWindow = getCurrentWindow();
    if (recorder.status === "recording") {
      appWindow.setSize(COMPACT_SIZE);
      appWindow.setAlwaysOnTop(true);
      appWindow.setDecorations(false);
      appWindow.setResizable(false);
      // Position to bottom-right of work area (physical pixels).
      // Small delay lets the resize settle before positioning.
      const MARGIN = 12;
      setTimeout(() => {
        Promise.all([getWorkArea(), appWindow.scaleFactor(), appWindow.outerSize()])
          .then(([area, scale, outerSize]) => {
            const margin = Math.round(MARGIN * scale);
            const x = area.x + area.width - outerSize.width - margin;
            const y = area.y + area.height - outerSize.height - margin;
            appWindow.setPosition(new PhysicalPosition(x, y));
          })
          .catch(() => {
            // Fallback: keep current position
          });
      }, 50);
    } else {
      appWindow.setSize(IDLE_SIZE);
      appWindow.setAlwaysOnTop(false);
      appWindow.setDecorations(true);
      appWindow.setResizable(false);
      appWindow.center();
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
    // Pre-emptive quota check: if we already know the user is at or over
    // their limit, show the rate-limit modal immediately without ever
    // touching the microphone. We also re-fetch quota first so a stale
    // client-side value does not block a legitimate recording.
    if (auth.loggedIn) {
      let latest = quotaHook.quota;
      if (!latest) {
        // Quota not loaded yet -- attempt a fresh fetch, but do not block
        // recording forever if the server is unreachable.
        await quotaHook.refresh();
        latest = quotaHook.quota;
      }
      if (latest && latest.remaining <= 0) {
        recorder.setRateLimited(latest.count, latest.limit);
        return;
      }
    }
    await recorder.start();
  }, [recorder, auth.loggedIn, quotaHook]);

  const handleStop = useCallback(async () => {
    await recorder.stop();
  }, [recorder]);

  const handleUndoLastScreenshot = useCallback(async () => {
    try {
      await deleteLastScreenshot();
      // Rust side decrements the counter; the useCaptureCount hook will
      // reflect the new value on the next step_captured event. To keep the
      // UI snappy we also optimistically re-render by reading the returned
      // count, but the hook-driven path is authoritative.
    } catch (e) {
      console.warn("Undo failed:", e);
    }
  }, []);

  const handleOpenFolder = useCallback(async () => {
    if (recorder.outputDir) {
      try {
        await revealItemInDir(recorder.outputDir);
      } catch (e) {
        console.error("Failed to open folder:", e);
      }
    }
  }, [recorder.outputDir]);

  // Retry-from-disk delegates to confirmGeneration, which is the single
  // source of truth for "run the pipeline against a stored outputDir and
  // walk the post-generation state machine". Previously this handler did
  // its own setProcessing + runGeneration but never transitioned to `done`
  // on success, leaving the UI stuck in a processing-busy state forever
  // even after the server returned the result and the PDF was saved.
  const handleRetry = useCallback(async () => {
    if (!recorder.outputDir) return;
    await recorder.confirmGeneration();
  }, [recorder]);

  // Loading state
  if (auth.loading) {
    return (
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100vh", background: "#14181C" }}>
        <p style={{ fontSize: 13, color: "#6B7780" }}>...</p>
      </div>
    );
  }

  const showBanner =
    !updater.dismissed &&
    recorder.status !== "recording" &&
    (updater.status === "available" || updater.status === "downloading");

  const updateBanner = showBanner ? (
      <div
        className="w-full flex items-center"
        style={{
          background: "linear-gradient(135deg, #1E8A93, #2CB5C0)",
          flexShrink: 0,
        }}
      >
        <button
          onClick={updater.install}
          disabled={updater.status === "downloading"}
          className="flex-1 py-2 text-xs font-medium text-center"
          style={{
            background: "transparent",
            color: "#fff",
            border: "none",
            cursor: updater.status === "downloading" ? "wait" : "pointer",
          }}
        >
          {updater.status === "downloading"
            ? t("update.downloading")
            : `${t("update.available", { version: updater.version ?? "" })} — ${t("update.install")}`}
        </button>
        {updater.status !== "downloading" && (
          <button
            onClick={updater.dismiss}
            className="px-2 py-2 text-xs"
            style={{
              background: "transparent",
              color: "rgba(255,255,255,0.7)",
              border: "none",
              cursor: "pointer",
              lineHeight: 1,
            }}
          >
            ✕
          </button>
        )}
      </div>
    ) : null;

  // Not logged in -> show login
  if (!auth.loggedIn) {
    return (
      <div className="flex flex-col h-full">
        {updateBanner}
        <div className="flex-1 min-h-0">
          <LoginScreen
            onLogin={auth.login}
            loading={auth.loading}
            error={auth.error}
            onOpenSettings={handleOpenSettings}
            version={version}
          />
        </div>
      </div>
    );
  }

  // Logged in -> show recorder
  return (
    <div className="flex flex-col h-full">
      {updateBanner}
      <div className="flex-1 min-h-0">
        <RecorderScreen
          email={auth.email}
          status={recorder.status}
          statusMessage={recorder.statusMessage}
          error={recorder.error}
          piiFindings={recorder.piiFindings}
          rateLimit={recorder.rateLimit}
          quota={quotaHook.quota}
          outputDir={recorder.outputDir}
          skipPiiCheck={skipPiiCheck}
          onStart={handleStart}
          onStop={handleStop}
          onCancel={recorder.cancel}
          onSignOut={auth.logout}
          onOpenSettings={handleOpenSettings}
          onOpenFolder={handleOpenFolder}
          onRetry={handleRetry}
          onDismissPii={() => recorder.setError(t("network.pii_blocked"))}
          onDismissRateLimit={recorder.dismissRateLimit}
          onUndoLastScreenshot={handleUndoLastScreenshot}
          onConfirmGeneration={recorder.confirmGeneration}
          onCancelFromReview={recorder.cancelFromReview}
          micPermission={micPermission}
          version={version}
        />
      </div>
    </div>
  );
}

export default App;
