import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

let stored: Record<string, unknown>;

beforeEach(() => {
  stored = {
    output_dir: "C:\\docs",
    logs_dir: "C:\\logs",
    hide_from_screenshots: true,
    upload_target: null,
    skip_pii_check: false,
    error_reports: "ask",
    capture_mode: "active_window",
    mask_passwords: true,
  };
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_settings") return { ...stored };
    if (cmd === "get_quota") return { features: {} };
    return;
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.resetModules();
});

// IS_MAC is read when the module loads, so each platform gets a fresh import.
async function renderOn(userAgent: string) {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(userAgent);
  const { SettingsPage } = await import("../components/SettingsPage");
  render(<SettingsPage isDev={false} />);
  await waitFor(() => expect(screen.getByText("Ganzer Bildschirm")).toBeInTheDocument());
}

const WINDOWS = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/130.0";
const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";

describe("capture privacy settings", () => {
  it("offers both modes on Windows with the monitor warning", async () => {
    await renderOn(WINDOWS);
    expect(screen.getByLabelText(/Nur das aktive Fenster/)).toBeChecked();
    expect(screen.getByLabelText(/Ganzer Bildschirm/)).not.toBeChecked();
    expect(
      screen.getByText(/Taskleiste, Benachrichtigungen und alle sichtbaren Fenster/),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Passwortfelder schwärzen")).toBeChecked();
    expect(screen.queryByText("Passwortfelder werden mit aufgenommen.")).not.toBeInTheDocument();
  });

  it("says plainly that password fields are captured when masking is off, and saves both", async () => {
    await renderOn(WINDOWS);
    const user = userEvent.setup();
    await user.click(screen.getByLabelText("Passwortfelder schwärzen"));
    expect(screen.getByText("Passwortfelder werden mit aufgenommen.")).toBeInTheDocument();
    await user.click(screen.getByLabelText(/Ganzer Bildschirm/));
    await user.click(screen.getByText("Speichern"));

    const save = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "save_settings");
    expect(save?.[1]).toMatchObject({
      settings: { capture_mode: "monitor", mask_passwords: false },
    });
  });

  it("offers only the monitor and no masking on macOS, with a note", async () => {
    await renderOn(MAC);
    expect(screen.queryByLabelText(/Nur das aktive Fenster/)).not.toBeInTheDocument();
    expect(screen.getByLabelText(/Ganzer Bildschirm/)).toBeChecked();
    expect(screen.queryByLabelText("Passwortfelder schwärzen")).not.toBeInTheDocument();
    expect(screen.getByText(/folgen später/)).toBeInTheDocument();
  });
});
