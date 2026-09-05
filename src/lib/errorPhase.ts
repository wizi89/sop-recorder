/**
 * What the user was doing when a failure happened (design D3's `phase`).
 *
 * A pure function on purpose. This used to be scattered across six
 * `set_phase` calls in Rust plus two hardcoded `"unknown"` strings in the
 * webview, and the result was that `Login`, `Review` and `Settings` were
 * defined, serialised, accepted by the server -- and produced by nobody. Every
 * webview error reported `unknown` and every panic outside a recording
 * reported `idle`. Nothing failed, so nothing caught it; it took reading a
 * report by hand.
 *
 * As one function with a table test, a phase that no screen can produce is a
 * failing test rather than a discovery months later. `phase` is a searchable
 * tag and, for everything except panics, part of the fingerprint
 * (`[kind, phase, normalised message]`), so a wrong one both mislabels the
 * report and groups it with unrelated failures.
 */
import type { RecorderStatus } from "../hooks/useRecorder";

/** Every phase the Rust `Phase` enum defines. Parity is asserted by a test. */
export const PHASES = [
  "startup",
  "login",
  "idle",
  "recording",
  "review",
  "processing",
  "settings",
  "unknown",
] as const;

export type Phase = (typeof PHASES)[number];

/**
 * Phases the webview is responsible for. `startup` belongs to Rust -- it is
 * set before the webview exists -- and `unknown` is the fallback for a report
 * raised before any screen has been established.
 */
export const WEBVIEW_PHASES: readonly Phase[] = [
  "login",
  "idle",
  "recording",
  "review",
  "processing",
  "settings",
];

export interface ScreenState {
  loggedIn: boolean;
  /** The settings window is open. It is a window of its own, and while it has
   *  focus it is what the user is doing, whatever the main window shows. */
  settingsOpen: boolean;
  /** The permission setup screen stands between login and the recorder. */
  permissionSetup: boolean;
  status: RecorderStatus;
}

export function phaseForScreen(state: ScreenState): Phase {
  // Checked before anything else: the settings window is on top of whatever
  // the main window is showing, so it is the honest answer.
  if (state.settingsOpen) return "settings";
  if (!state.loggedIn) return "login";
  // Permission setup is part of getting started, not a phase of its own; it
  // sits where the recorder would be and reports as idle.
  if (state.permissionSetup) return "idle";

  switch (state.status) {
    case "recording":
      return "recording";
    case "review":
    case "done":
      // `done` is the review screen with a finished guide on it.
      return "review";
    case "processing":
      return "processing";
    case "idle":
    case "error":
    case "pii_blocked":
    case "rate_limited":
      // An error is shown on the screen the user was already on, and all three
      // of these are states of the idle recorder screen.
      return "idle";
  }
}
