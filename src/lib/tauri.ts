import { invoke } from "@tauri-apps/api/core";
import type { Phase } from "./errorPhase";

export interface SessionState {
  logged_in: boolean;
  email: string | null;
}

/** How the recorder handles error reports. `ask` shows the dialog for every
 *  report, `always` sends without one, `never` collects nothing at all. */
export type ErrorReportMode = "ask" | "always" | "never";

export interface AppSettings {
  output_dir: string;
  logs_dir: string;
  hide_from_screenshots: boolean;
  api_key: string | null;
  upload_target: string | null;
  skip_pii_check: boolean;
  pipeline_version: number;
  generation_model: string;
  error_reports: ErrorReportMode;
}

export async function login(
  email: string,
  password: string,
): Promise<SessionState> {
  return invoke("login", { email, password });
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function refreshSession(): Promise<SessionState> {
  return invoke("refresh_session");
}

export async function getSessionState(): Promise<SessionState> {
  return invoke("get_session_state");
}

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke("save_settings", { settings });
}

export async function startRecording(): Promise<void> {
  return invoke("start_recording");
}

export async function stopRecording(): Promise<string> {
  return invoke("stop_recording");
}

/**
 * Delete the most recently captured screenshot from the active recording
 * session. Returns the new screenshot count after deletion.
 * Throws if no session is active, counter is already 0, or a capture is
 * currently in-flight.
 */
export async function deleteLastScreenshot(): Promise<number> {
  return invoke("delete_last_screenshot");
}

/**
 * List all captured screenshot files in a recording session output dir,
 * ordered by capture sequence. Returns absolute file paths.
 */
export async function listSessionScreenshots(outputDir: string): Promise<string[]> {
  return invoke("list_session_screenshots", { outputDir });
}

/**
 * Read a screenshot file as raw bytes. The frontend wraps the result in a
 * Blob and creates an object URL for display -- this sidesteps the need to
 * configure Tauri's asset protocol to allow arbitrary filesystem paths.
 */
export async function readScreenshotBytes(path: string): Promise<Uint8Array> {
  const bytes = await invoke<number[]>("read_screenshot_bytes", { path });
  return new Uint8Array(bytes);
}

/**
 * `undetermined` means the OS has never asked. It is worth keeping apart from
 * `denied`, because only `undetermined` can be resolved by prompting: macOS
 * shows the dialog once and, after a refusal, the request call does nothing at
 * all. A screen that treats the two alike offers a button that cannot work.
 */
export type MicPermissionState =
  | "granted"
  | "denied"
  | "undetermined"
  | "unknown";
/**
 * No `undetermined` here: `CGPreflightScreenCaptureAccess` answers yes or no
 * and macOS exposes nothing finer, so a refusal is indistinguishable from a
 * question never asked. Both are handled the same way -- send the user to
 * System Settings, which is where this grant is made either way.
 */
export type ScreenRecordingPermissionState = "granted" | "denied" | "unknown";
/**
 * Whether the global input hook can be installed. macOS only: without the
 * Accessibility grant the recorder runs and captures no steps at all.
 */
export type AccessibilityPermissionState = "granted" | "denied" | "unknown";

function normalizePermission<
  T extends "granted" | "denied" | "undetermined" | "unknown",
>(state: string): T {
  if (state === "granted" || state === "denied" || state === "undetermined") {
    return state as T;
  }
  return "unknown" as T;
}

export async function getMicrophonePermissionState(): Promise<MicPermissionState> {
  try {
    return normalizePermission<MicPermissionState>(
      await invoke<string>("get_microphone_permission_state"),
    );
  } catch {
    return "unknown";
  }
}

export async function getScreenRecordingPermissionState(): Promise<ScreenRecordingPermissionState> {
  try {
    return normalizePermission<ScreenRecordingPermissionState>(
      await invoke<string>("get_screen_recording_permission_state"),
    );
  } catch {
    return "unknown";
  }
}

export async function getAccessibilityPermissionState(): Promise<AccessibilityPermissionState> {
  try {
    return normalizePermission<AccessibilityPermissionState>(
      await invoke<string>("get_accessibility_permission_state"),
    );
  } catch {
    return "unknown";
  }
}



/**
 * Relaunch the app so newly granted macOS permissions take effect.
 * Screen Recording and Accessibility apply only to a fresh process.
 */
export async function restartApp(): Promise<void> {
  return invoke("restart_app");
}

export async function runGeneration(outputDir: string): Promise<void> {
  return invoke("run_generation", { outputDir });
}

export interface OrgFeatures {
  advanced_settings: boolean;
}

export interface Quota {
  count: number;
  limit: number;
  remaining: number;
  features: OrgFeatures;
  generation_settings?: GenerationSettings;
}

export interface GenerationSettings {
  pipeline_versions: number[];
  models: string[];
  default_model: string;
}

export async function getQuota(): Promise<Quota> {
  return invoke("get_quota");
}

export async function getWebappUrl(): Promise<string> {
  return invoke("get_webapp_url");
}

/**
 * One selectable generation pipeline. `display_name` and `description` are
 * user-facing copy authored on the server; `id` is the only value ever sent
 * back with an upload.
 */
export interface Pipeline {
  id: string;
  display_name: string;
  description: string;
}

/**
 * Fetch the pipeline catalogue. Never rejects on a server or network failure:
 * the command falls back to the last good catalogue, or an empty list. An
 * empty list means "no selector", not "something went wrong".
 */
export async function getPipelines(): Promise<Pipeline[]> {
  return invoke("get_pipelines");
}

/** Read the remembered pipeline choice. Empty string means none. */
export async function getSelectedPipeline(): Promise<string> {
  return invoke("get_selected_pipeline");
}

/** Remember a pipeline choice for next time. Empty string clears it. */
export async function setSelectedPipeline(pipelineId: string): Promise<void> {
  return invoke("set_selected_pipeline", { pipelineId });
}

export interface WorkArea {
  x: number;
  y: number;
  width: number;
  height: number;
}

export async function getWorkArea(): Promise<WorkArea> {
  return invoke("get_work_area");
}

/**
 * Tell the capture side where the compact recording bar is, as
 * [x, y, width, height] in logical points from the top-left of the primary
 * display, or `null` when it is not on screen.
 *
 * Clicks inside it are the user driving the recorder -- pressing Stop -- and
 * must not become steps of the process being recorded.
 */
export async function setRecorderRegion(
  region: [number, number, number, number] | null,
): Promise<void> {
  return invoke("set_recorder_region", { region });
}

/**
 * Keep the compact recording bar visible above everything, including apps in
 * native fullscreen, or hand the window back its ordinary behaviour.
 *
 * Replaces `setAlwaysOnTop` for the recording lifecycle. Always-on-top only
 * changes the window's level, which orders it within one Space; a fullscreen
 * app gets a Space of its own, so the bar disappeared exactly when the user
 * still needed to reach Stop. Screenshot exclusion is unaffected -- that is
 * `setDisplayAffinity`, a separate property.
 *
 * Scoped to the recording: the idle window must not float over everything.
 */
export async function setOverlayMode(enabled: boolean): Promise<void> {
  return invoke("set_overlay_mode", { enabled });
}

/**
 * Open the System Settings pane for a refused permission.
 *
 * Goes through a command rather than the opener plugin: that plugin's default
 * scope is `http`, `https`, `mailto` and `tel`, so the
 * `x-apple.systempreferences:` URL was rejected before reaching the OS and the
 * link silently did nothing. The Rust side matches `pane` against a fixed
 * allowlist.
 */
export async function openPrivacySettings(
  pane: "microphone" | "screen" | "accessibility",
): Promise<void> {
  return invoke("open_privacy_settings", { pane });
}

export type PermissionName = "microphone" | "screen" | "accessibility";

/**
 * Raise the OS prompt for one permission and return where it stands after.
 *
 * Per permission rather than all at once, because macOS shows a dialog only
 * while a permission is undetermined. A single "grant everything" button could
 * not keep that promise once one had been refused: it did nothing for that one
 * and gave no hint why.
 */
export async function requestPermission(
  which: PermissionName,
): Promise<string> {
  return invoke<string>("request_permission", { which });
}

// -- Error reports --

/** What a report may contain. Mirrors `ErrorReport` in `error_reports.rs`;
 *  the fields it lacks are the guarantee -- there is no screenshot, audio,
 *  transcript, guide, email, token or output path field anywhere in it. */
export interface ErrorReport {
  schema_version: number;
  report_id: string;
  kind: "panic" | "command_error" | "ui_error";
  occurred_at: string;
  app_version: string;
  os: string;
  os_version: string;
  arch: string;
  locale: string;
  phase: string;
  message: string;
  location: string | null;
  log_tail: string[];
  settings: {
    upload_target: string | null;
    pipeline_version: number;
    generation_model: string;
    hide_from_screenshots: boolean;
    skip_pii_check: boolean;
  } | null;
  job_id: string | null;
  comment: string | null;
  consent: "pending" | "granted";
}

export interface SubmittedReport {
  report_id: string;
  number: string;
}

/** The event the Rust side emits when a report appears, carrying its id. A
 *  panic on a thread other than the main one leaves the process running, so
 *  the dialog should not wait for a relaunch that may never come. */
export const ERROR_REPORT_CREATED = "error_report:created";

/** Every report still waiting on disk, oldest first. Empty when reports are
 *  switched off. */
export async function listErrorReports(): Promise<ErrorReport[]> {
  return invoke("list_error_reports");
}

export async function readErrorReport(
  reportId: string,
): Promise<ErrorReport | null> {
  return invoke("read_error_report", { reportId });
}

/** Create a report for a failure the webview saw. Answers null when reports
 *  are switched off, which is not an error. */
export async function createErrorReport(
  kind: "command_error" | "ui_error",
  phase: string,
  message: string,
  jobId?: string | null,
): Promise<ErrorReport | null> {
  return invoke("create_error_report", {
    kind,
    phase,
    message,
    jobId: jobId ?? null,
  });
}

/** Record the user's answer. Declining deletes the file; nothing has been
 *  transmitted at that point, and nothing will be. */
export async function decideErrorReport(
  reportId: string,
  grant: boolean,
  comment?: string | null,
): Promise<ErrorReport | null> {
  return invoke("decide_error_report", {
    reportId,
    grant,
    comment: comment ?? null,
  });
}

/** The absolute path of a report's file, for revealing it in the file
 *  manager. The webview cannot build this itself -- the reports directory is
 *  resolved on the Rust side and differs per platform. */
export async function errorReportPath(reportId: string): Promise<string> {
  return invoke("error_report_path", { reportId });
}

/** Send every granted report the current session can carry. A report created
 *  while signed out waits for this to run after the next sign-in. */
export async function submitErrorReports(): Promise<SubmittedReport[]> {
  return invoke("submit_error_reports");
}

/** Whether the installation has switched error reports off. */
export async function areErrorReportsForcedOff(): Promise<boolean> {
  try {
    return await invoke<boolean>("are_error_reports_forced_off");
  } catch {
    return false;
  }
}

/**
 * Force a failure on purpose, for testing the report flow by hand.
 *
 * Dev builds only: the Rust side gates its bodies on `debug_assertions` and
 * answers with a refusal in a release binary. Only reachable from the dev-only
 * buttons on the settings page.
 */
export async function debugTriggerFailure(kind: string): Promise<void> {
  await invoke("debug_trigger_failure", { kind });
}

/**
 * The phase last published to the Rust side.
 *
 * Mirrored here because the two global error handlers in `main.tsx` and the
 * React error boundary are synchronous and outside the component tree, so they
 * cannot read React state or await a command. They used to pass the literal
 * `"unknown"`, which is why every webview error arrived with no idea what the
 * user was doing.
 */
let currentPhase: Phase = "unknown";

/** What the webview last told the Rust side the user was doing. */
export function errorReportPhase(): Phase {
  return currentPhase;
}

/**
 * Publish the current screen as the report phase.
 *
 * Idempotent and cheap: repeats are dropped, so callers can fire it from an
 * effect on every render without thinking about it.
 */
export async function setErrorReportPhase(phase: Phase, force = false): Promise<void> {
  // Each webview is its own JS context with its own mirror, but they share one
  // phase in Rust. So when the settings window sets `settings` and closes, the
  // main window's mirror still says what it said before and the dedup would
  // skip the restore. `force` is how the main window reclaims the phase on
  // regaining focus.
  if (phase === currentPhase && !force) return;
  currentPhase = phase;
  try {
    await invoke("set_error_report_phase", { phase });
  } catch (e) {
    // A phase that did not land is a mislabelled report, never a lost one.
    console.warn("Phase für Fehlerberichte konnte nicht gesetzt werden:", e);
  }
}
