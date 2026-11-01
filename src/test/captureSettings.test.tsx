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
  it("offers both areas on Windows, active window first, with what it captures", async () => {
    await renderOn(WINDOWS);
    const area = screen.getByLabelText("Aufnahmebereich") as HTMLSelectElement;
    expect(area.value).toBe("active_window");
    expect([...area.options].map((o) => o.text)).toEqual(["Aktives Fenster", "Ganzer Bildschirm"]);
    expect(screen.getByText(/Gespeichert wird das Fenster, in dem geklickt wird/)).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Passwortfelder schwärzen" })).toBeChecked();
    expect(screen.queryByText("Passwortfelder werden mit aufgenommen.")).not.toBeInTheDocument();
  });

  it("warns about the monitor, says plainly that password fields are captured when masking is off, and saves both", async () => {
    await renderOn(WINDOWS);
    const user = userEvent.setup();
    await user.click(screen.getByRole("switch", { name: "Passwortfelder schwärzen" }));
    expect(screen.getByText("Passwortfelder werden mit aufgenommen.")).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("Aufnahmebereich"), "monitor");
    expect(
      screen.getByText(/Taskleiste, Benachrichtigungen und alle sichtbaren Fenster/),
    ).toBeInTheDocument();
    await user.click(screen.getByText("Speichern"));

    const save = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "save_settings");
    expect(save?.[1]).toMatchObject({
      settings: { capture_mode: "monitor", mask_passwords: false },
    });
  });

  it("offers only the monitor and no masking on macOS, with a note", async () => {
    await renderOn(MAC);
    const area = screen.getByLabelText("Aufnahmebereich") as HTMLSelectElement;
    expect([...area.options].map((o) => o.value)).toEqual(["monitor"]);
    expect(area).toBeDisabled();
    expect(screen.queryByRole("switch", { name: "Passwortfelder schwärzen" })).not.toBeInTheDocument();
    expect(screen.getByText(/folgen später/)).toBeInTheDocument();
  });
});
