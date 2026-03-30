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

export async function runGeneration(outputDir: string): Promise<void> {
  return invoke("run_generation", { outputDir });
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
