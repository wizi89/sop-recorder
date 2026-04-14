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

/**
 * Query the microphone permission state at app launch. Returns one of
 * `"granted" | "denied" | "unknown"`. On `denied` the UI should render a
 * warning chip telling the user to resolve the issue before recording.
 */
export async function getMicrophonePermissionState(): Promise<MicPermissionState> {
  try {
    const state = await invoke<string>("get_microphone_permission_state");
    if (state === "granted" || state === "denied") return state;
    return "unknown";
  } catch {
    return "unknown";
  }
}

export async function runGeneration(outputDir: string): Promise<void> {
  return invoke("run_generation", { outputDir });
}

export interface Quota {
  count: number;
  limit: number;
  remaining: number;
}

export async function getQuota(): Promise<Quota> {
  return invoke("get_quota");
}

export async function getWebappUrl(): Promise<string> {
  return invoke("get_webapp_url");
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
