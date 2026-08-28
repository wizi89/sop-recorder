import { invoke } from "@tauri-apps/api/core";

export interface SessionState {
  logged_in: boolean;
  email: string | null;
}

export interface AppSettings {
  output_dir: string;
  logs_dir: string;
  hide_from_screenshots: boolean;
  api_key: string | null;
  upload_target: string | null;
  skip_pii_check: boolean;
  pipeline_version: number;
  generation_model: string;
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

export type MicPermissionState = "granted" | "denied" | "unknown";
export type ScreenRecordingPermissionState = "granted" | "denied" | "unknown";
/**
 * Whether the global input hook can be installed. macOS only: without the
 * Accessibility grant the recorder runs and captures no steps at all.
 */
export type AccessibilityPermissionState = "granted" | "denied" | "unknown";

function normalizePermission<T extends "granted" | "denied" | "unknown">(state: string): T {
  if (state === "granted" || state === "denied") return state as T;
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

export interface PermissionsState {
  microphone: MicPermissionState;
  screen_recording: ScreenRecordingPermissionState;
  accessibility: AccessibilityPermissionState;
}

/**
 * Trigger the macOS TCC prompts for mic, screen recording and accessibility in
 * one batch. No-op on Windows (returns `granted` for all three).
 *
 * Accessibility reports `denied` on the run that first prompts: macOS opens
 * System Settings for it rather than deciding in the dialog.
 */
export async function requestAllPermissions(): Promise<PermissionsState> {
  const raw = await invoke<{
    microphone: string;
    screen_recording: string;
    accessibility: string;
  }>("request_all_permissions");
  return {
    microphone: normalizePermission<MicPermissionState>(raw.microphone),
    screen_recording: normalizePermission<ScreenRecordingPermissionState>(raw.screen_recording),
    accessibility: normalizePermission<AccessibilityPermissionState>(raw.accessibility),
  };
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
