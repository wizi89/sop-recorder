import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve("0.15.0")),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
    show: vi.fn(),
    hide: vi.fn(),
    setFocus: vi.fn(),
    close: vi.fn(),
    scaleFactor: vi.fn(() => Promise.resolve(1)),
    outerSize: vi.fn(() => Promise.resolve({ width: 240, height: 34 })),
    setPosition: vi.fn(),
  })),
  PhysicalPosition: vi.fn(),
  LogicalSize: vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: Object.assign(vi.fn(), {
    getByLabel: vi.fn(() => Promise.resolve(null)),
  }),
}));

/// The browser sign-in is a five-minute wait, and `loading` is true for all of
/// it. App's own loading gate used to win, so the user sat in front of "..."
/// with the cancel button and the hint rendered nowhere.
describe("waiting for the browser (task 7.4)", () => {
  let cancelled = false;

  beforeEach(() => {
    cancelled = false;
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case "refresh_session":
          return { logged_in: false, email: null };
        case "login_via_browser":
          // What the real command does for as long as the user is in the
          // browser: nothing. The wait is the state under test.
          return new Promise(() => {});
        case "cancel_browser_login":
          cancelled = true;
          return null;
        case "get_microphone_permission_state":
        case "get_screen_recording_permission_state":
        case "get_accessibility_permission_state":
          return "granted";
        case "list_error_reports":
          return [];
        default:
          return null;
      }
    });
  });

  it("shows the wait and its cancel button, not the loading screen", async () => {
    const { default: App } = await import("../App");
    render(<App />);

    const start = await screen.findByRole("button", {
      name: /mit browser anmelden/i,
    });
    await userEvent.click(start);

    await waitFor(() =>
      expect(screen.getByText(/Warte auf die Anmeldung im Browser/)).toBeInTheDocument(),
    );

    const cancel = screen.getByRole("button", { name: "Abbrechen" });
    await userEvent.click(cancel);
    await waitFor(() => expect(cancelled).toBe(true));
  });
});
