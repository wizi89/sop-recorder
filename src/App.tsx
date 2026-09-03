import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LoginScreen } from "./components/LoginScreen";
import { PermissionsScreen } from "./components/PermissionsScreen";
import { RecorderScreen } from "./components/RecorderScreen";
import { SettingsPage } from "./components/SettingsPage";
import { ErrorReportModal } from "./components/ErrorReportModal";
import { useAuth } from "./hooks/useAuth";
import { useRecorder } from "./hooks/useRecorder";
import { useErrorReports } from "./hooks/useErrorReports";
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
  restartApp,
  saveSettings,
  type ErrorReportMode,
  type MicPermissionState,
  type ScreenRecordingPermissionState,
  type AccessibilityPermissionState,
} from "./lib/tauri";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { ask, open } from "@tauri-apps/plugin-dialog";
const IS_DEV = import.meta.env.DEV;

// Must match the label `create_recording_bar` builds the window under.
const BAR_LABEL = "bar";

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
  const [errorReportMode, setErrorReportMode] = useState<ErrorReportMode>("ask");
  const [errorReportNotice, setErrorReportNotice] = useState<string | null>(null);
  const [micPermission, setMicPermission] = useState<MicPermissionState>("unknown");
  const [screenRecordingPermission, setScreenRecordingPermission] =
    useState<ScreenRecordingPermissionState>("unknown");
  const [accessibilityPermission, setAccessibilityPermission] =
    useState<AccessibilityPermissionState>("unknown");
  // The first-run screen is dismissed for this launch, not forever: the
  // recorder's banner keeps carrying whatever is still missing.
  const [permissionSetupSkipped, setPermissionSetupSkipped] = useState(false);
  const { t } = useTranslation();
  const auth = useAuth();
  const recorder = useRecorder();
  const updater = useUpdater();
  // Error reports (design D1, D5, D7). The hook owns the queue; this window
  // owns what is on screen. Under mode `always` no dialog opens and the number
  // goes into the status bar instead.
  const handleAutoSent = useCallback(
    (number: string) => setErrorReportNotice(t("report.auto_sent", { number })),
    [t],
  );
  const errorReports = useErrorReports({
    loggedIn: auth.loggedIn,
    mode: errorReportMode,
    onAutoSent: handleAutoSent,
  });
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
      .then((s) => {
        setSkipPiiCheck(s.skip_pii_check);
        setErrorReportMode(s.error_reports);
      })
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
    // Returned as well as stored: a caller that needs the answer now cannot
    // read state it set in the same tick.
    return { mic, screen, accessibility };
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

  // Reopen the permission setup rather than firing prompts from here.
  //
  // The banner used to call a batch request, which could not help in the one
  // state it was ever shown in: macOS raises a permission dialog only while a
  // permission is undetermined, and a banner about a *denied* permission is by
  // definition past that. Worse, firing three prompts at once meant only one
  // dialog could be presented and the rest were auto-denied. The setup screen
  // carries the per-permission action that does work.
  const handleOpenPermissionSetup = useCallback(() => {
    setPermissionSetupSkipped(false);
  }, []);

  useEffect(() => {
    if (!errorReportNotice) return;
    const id = setTimeout(() => setErrorReportNotice(null), 6000);
    return () => clearTimeout(id);
  }, [errorReportNotice]);

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
    // Nothing between app start and here re-reads the permissions, so one
    // revoked mid-session was invisible until the audio gave it away five
    // seconds in -- and a revoked Screen Recording gave nothing away at all,
    // it just filled the guide with pictures of the desktop wallpaper.
    const perms = await refreshPermissions();

    // Screen Recording and Accessibility are refused outright: without either
    // the recording cannot produce anything worth keeping. Missing screen
    // capture yields wallpaper screenshots; missing accessibility captures no
    // steps at all, while the timer runs as though it were working.
    const blocking: string[] = [];
    if (perms.screen !== "granted") blocking.push(t("permissions.screen_title"));
    if (perms.accessibility !== "granted") {
      blocking.push(t("permissions.accessibility_title"));
    }
    if (blocking.length > 0) {
      recorder.setError(
        t("permissions.blocked_start", { names: blocking.join(", ") }),
      );
      return;
    }

    // The microphone only degrades the result -- the steps and screenshots are
    // still captured -- and recording without narration is a legitimate thing
    // to want. So it asks rather than refuses.
    if (perms.mic !== "granted") {
      const proceed = await ask(t("permissions.no_mic_confirm"), {
        title: t("permissions.no_mic_title"),
        kind: "warning",
        okLabel: t("permissions.no_mic_continue"),
        cancelLabel: t("status.cancel"),
      });
      if (!proceed) return;
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
  }, [recorder, auth.loggedIn, quotaHook, refreshPermissions, t]);

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

  // The bar's three controls, arriving from the other window.
  //
  // The bar deliberately holds no recording state: it emits, and this window --
  // hidden but still running -- performs the same actions it always did. One
  // owner of the recorder means the two webviews cannot disagree about what is
  // happening, and the bar stays a remote control rather than a second brain.
  useEffect(() => {
    const unlisten = Promise.all([
      listen("bar:stop", () => void handleStop()),
      listen("bar:cancel", () => void recorder.cancel()),
      listen("bar:undo", () => void handleUndoLastScreenshot()),
    ]);
    return () => {
      void unlisten.then((fns) => fns.forEach((fn) => fn()));
    };
  }, [handleStop, handleUndoLastScreenshot, recorder]);

  // The error state carries a button, not a modal: the user is already looking
  // at an error and choosing what to do next, and a modal on top of it would be
  // the second interruption in a second (design D6).
  const handleSendErrorReport = useCallback(async () => {
    if (!recorder.reportableError) return;
    await errorReports.create(
      "command_error",
      recorder.status === "processing" ? "processing" : "idle",
      recorder.reportableError,
    );
  }, [errorReports, recorder.reportableError, recorder.status]);

  const handleGrantReport = useCallback(
    async (comment: string | null, alwaysSend: boolean) => {
      const report = errorReports.current;
      if (!report) return;
      if (alwaysSend) {
        // The checkbox is a settings change, so it survives the session and
        // the settings page shows it.
        try {
          const current = await getSettings();
          await saveSettings({ ...current, error_reports: "always" });
          setErrorReportMode("always");
        } catch (e) {
          console.warn("Modus für Fehlerberichte konnte nicht gespeichert werden:", e);
        }
      }
      await errorReports.grant(report, comment);
    },
    [errorReports],
  );

  const handleDeclineReport = useCallback(async () => {
    const report = errorReports.current;
    if (!report) return;
    await errorReports.decline(report);
  }, [errorReports]);

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

  // Mounted outside every screen below, so a report found at launch is asked
  // about even on the login screen -- a failed sign-in is the first error most
  // testers meet, and it is the one that cannot be reported from inside a
  // session (design D7).
  const errorReportModal =
    errorReports.current || errorReports.sent ? (
      <ErrorReportModal
        report={errorReports.current}
        loggedIn={auth.loggedIn}
        sent={errorReports.sent}
        onGrant={handleGrantReport}
        onDecline={handleDeclineReport}
        onClose={errorReports.dismissSent}
      />
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
        {errorReportModal}
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
            onRestart={() => void restartApp()}
            onSkip={() => setPermissionSetupSkipped(true)}
          />
        </div>
        {errorReportModal}
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
          onSignOut={auth.logout}
          onOpenSettings={handleOpenSettings}
          onOpenFolder={handleOpenFolder}
          onRetry={handleRetry}
          onDismissPii={() => recorder.setError(t("network.pii_blocked"))}
          onDismissRateLimit={recorder.dismissRateLimit}
          onConfirmGeneration={recorder.confirmGeneration}
          onCancelFromReview={recorder.cancelFromReview}
          onGenerateFromFolder={handleGenerateFromFolder}
          micPermission={micPermission}
          screenRecordingPermission={screenRecordingPermission}
          accessibilityPermission={accessibilityPermission}
          onRequestPermissions={handleOpenPermissionSetup}
          onSendErrorReport={
            recorder.reportableError ? () => void handleSendErrorReport() : undefined
          }
          version={version}
        />
      </div>
      {/* Mode `always`: no dialog, a line in the status bar with the number
          the tracker can be searched by (design D1). */}
      {errorReportNotice && (
        <div
          className="px-4 pb-2 text-center"
          style={{ fontSize: "0.625rem", color: "#6B7780" }}
        >
          {errorReportNotice}
        </div>
      )}
      {errorReportModal}
    </div>
  );
}

export default App;
