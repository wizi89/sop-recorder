import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LoginScreen } from "./components/LoginScreen";
import { PermissionsScreen } from "./components/PermissionsScreen";
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
  setRecorderRegion,
  getSettings,
  deleteLastScreenshot,
  listSessionScreenshots,
  getMicrophonePermissionState,
  getScreenRecordingPermissionState,
  getAccessibilityPermissionState,
  requestAllPermissions,
  restartApp,
  type MicPermissionState,
  type ScreenRecordingPermissionState,
  type AccessibilityPermissionState,
} from "./lib/tauri";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { open } from "@tauri-apps/plugin-dialog";
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
  const [screenRecordingPermission, setScreenRecordingPermission] =
    useState<ScreenRecordingPermissionState>("unknown");
  const [accessibilityPermission, setAccessibilityPermission] =
    useState<AccessibilityPermissionState>("unknown");
  // The first-run screen is dismissed for this launch, not forever: the
  // recorder's banner keeps carrying whatever is still missing.
  const [permissionSetupSkipped, setPermissionSetupSkipped] = useState(false);
  const [requestingPermissions, setRequestingPermissions] = useState(false);
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

  const refreshPermissions = useCallback(async () => {
    const [mic, screen, accessibility] = await Promise.all([
      getMicrophonePermissionState(),
      getScreenRecordingPermissionState(),
      getAccessibilityPermissionState(),
    ]);
    setMicPermission(mic);
    setScreenRecordingPermission(screen);
    setAccessibilityPermission(accessibility);
  }, []);

  useEffect(() => {
    getVersion().then(setVersion);
    loadSettings();
    // Probe permission states up-front so the UI can surface the setup screen
    // before the user attempts to record. On macOS without these the recorder
    // either fails (mic), silently captures the wallpaper (screen recording),
    // or runs a recording that captures no steps whatsoever (accessibility,
    // which is what the global input hook needs).
    void refreshPermissions();
  }, [loadSettings, refreshPermissions]);

  // Screen Recording and Accessibility are granted in System Settings, not in
  // the dialog, so the app is not told when it happens. Poll while the setup
  // screen is up, and only while it is up, so returning from System Settings
  // updates the rows instead of leaving them stale. Stops as soon as
  // everything is granted, or the screen is dismissed.
  const permissionsAllGranted =
    micPermission === "granted" &&
    screenRecordingPermission === "granted" &&
    accessibilityPermission === "granted";
  const permissionSetupVisible = !permissionsAllGranted && !permissionSetupSkipped;

  useEffect(() => {
    if (!permissionSetupVisible) return;
    const id = setInterval(() => void refreshPermissions(), 1500);
    return () => clearInterval(id);
  }, [permissionSetupVisible, refreshPermissions]);

  // Fire all macOS TCC prompts in one batch from a single user gesture,
  // then re-read state so the banner clears without a relaunch.
  const handleRequestPermissions = useCallback(async () => {
    setRequestingPermissions(true);
    try {
      const next = await requestAllPermissions();
      setMicPermission(next.microphone);
      setScreenRecordingPermission(next.screen_recording);
      setAccessibilityPermission(next.accessibility);
    } catch (e) {
      console.warn("Permission bootstrap failed:", e);
    } finally {
      setRequestingPermissions(false);
    }
  }, []);

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

  /// Shrink to the recording bar, anchor it to the corner of the work area,
  /// and tell the capture side where it landed.
  ///
  /// Awaited before the recording starts rather than reacted to afterwards.
  /// The input hook goes live inside `start_recording`, so anchoring after it
  /// left a window in which a click on the bar was still captured as a step --
  /// the exact defect the region exists to prevent.
  const enterCompactMode = useCallback(async () => {
    const appWindow = getCurrentWindow();
    await appWindow.setSize(COMPACT_SIZE);
    await appWindow.setAlwaysOnTop(true);
    await appWindow.setDecorations(false);
    await appWindow.setResizable(false);

    const MARGIN = 12;
    try {
      const [area, scale, outerSize] = await Promise.all([
        getWorkArea(),
        appWindow.scaleFactor(),
        appWindow.outerSize(),
      ]);
      const margin = Math.round(MARGIN * scale);
      const x = area.x + area.width - outerSize.width - margin;
      const y = area.y + area.height - outerSize.height - margin;
      await appWindow.setPosition(new PhysicalPosition(x, y));
      // In the logical points the input hook reports cursor positions in.
      await setRecorderRegion([
        Math.round(x / scale),
        Math.round(y / scale),
        Math.round(outerSize.width / scale),
        Math.round(outerSize.height / scale),
      ]);
    } catch (e) {
      // Position unknown: leave the window where it is and report no region,
      // so a stale one cannot swallow real clicks.
      console.warn("Could not anchor the recording bar:", e);
      await setRecorderRegion(null);
    }
  }, []);

  const exitCompactMode = useCallback(async () => {
    const appWindow = getCurrentWindow();
    await setRecorderRegion(null);
    await appWindow.setSize(IDLE_SIZE);
    await appWindow.setAlwaysOnTop(false);
    await appWindow.setDecorations(true);
    await appWindow.setResizable(false);
    await appWindow.center();
  }, []);

  // Restores the full window when a recording ends. Entering compact mode is
  // done by the caller before starting, not here, so the bar is already in
  // place and its region already reported when the input hook goes live.
  useEffect(() => {
    if (recorder.status !== "recording") {
      void exitCompactMode();
    }
  }, [recorder.status, exitCompactMode]);

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
    // Anchor the bar and register its region BEFORE the hook starts, so the
    // click that stops the recording can never be captured as a step.
    await enterCompactMode();
    try {
      await recorder.start();
    } catch (e) {
      // Never leave the user stranded in a 240x34 window with no recording.
      await exitCompactMode();
      throw e;
    }
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

  // Pick an existing recording folder and run the full generation pipeline
  // against it, reusing login + the currently selected model/pipeline + upload
  // + persistence. Lets a user regenerate from a previously captured recording
  // without re-recording (e.g. retry with a different model/pipeline). The
  // folder must contain `recording.wav` and a `screenshots/` directory (a
  // recorder session dir).
  const handleGenerateFromFolder = useCallback(async () => {
    try {
      const dir = await open({
        directory: true,
        multiple: false,
        title: "Aufnahme-Ordner wählen",
      });
      if (typeof dir === "string") {
        // Guard: make sure the picked folder is actually a recording session
        // (has captured screenshots) before kicking off a real generation, so a
        // wrong folder fails fast with a friendly message instead of a cryptic
        // pipeline error after upload. list_session_screenshots returns [] when
        // there is no screenshots/ subdir.
        const shots = await listSessionScreenshots(dir);
        if (!shots || shots.length === 0) {
          recorder.setError(t("status.invalid_recording_folder"));
          return;
        }
        await recorder.generateFromDir(dir);
      }
    } catch (e) {
      console.error("Generate from folder failed:", e);
      recorder.setError(t("status.invalid_recording_folder"));
    }
  }, [recorder, t]);

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
            version={version}
          />
        </div>
      </div>
    );
  }

  // Logged in, but the OS has not granted what a recording needs -> set that
  // up first, in one sitting, rather than interrupting the first recording.
  if (permissionSetupVisible) {
    return (
      <div className="flex flex-col h-full">
        {updateBanner}
        <div className="flex-1 min-h-0">
          <PermissionsScreen
            micPermission={micPermission}
            screenRecordingPermission={screenRecordingPermission}
            accessibilityPermission={accessibilityPermission}
            onRequestPermissions={handleRequestPermissions}
            onRestart={() => void restartApp()}
            onSkip={() => setPermissionSetupSkipped(true)}
            requesting={requestingPermissions}
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
          onGenerateFromFolder={handleGenerateFromFolder}
          micPermission={micPermission}
          screenRecordingPermission={screenRecordingPermission}
          accessibilityPermission={accessibilityPermission}
          onRequestPermissions={handleRequestPermissions}
          version={version}
        />
      </div>
    </div>
  );
}

export default App;
