import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SettingsPage } from "../components/SettingsPage";

const LOG_DIR = "/Users/m/Library/Logs/com.cogniclone.recorder";

const stored = {
  output_dir: "C:\\docs\\workflows",
  logs_dir: LOG_DIR,
  hide_from_screenshots: true,
  upload_target: null,
  skip_pii_check: false,
  pipeline_version: 1,
  generation_model: "azure/gpt-4.1",
  error_reports: "ask",
};

let saveRejects = false;
// The shared mock hands back a fresh object per call, so the close() the
// component used would not be the one an assertion inspects.
const win = { setSize: vi.fn(), setAlwaysOnTop: vi.fn(), setDecorations: vi.fn(),
              setResizable: vi.fn(), close: vi.fn() };

beforeEach(() => {
  saveRejects = false;
  win.close.mockClear();
  vi.mocked(getCurrentWindow).mockReturnValue(win as never);
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_settings") return { ...stored };
    if (cmd === "save_settings") {
      if (saveRejects) throw new Error("Zugriff verweigert");
      return;
    }
    if (cmd === "get_quota") {
      return { count: 0, limit: 100, remaining: 100, features: { advanced_settings: false } };
    }
    if (cmd === "are_error_reports_forced_off") return false;
    return;
  });
});

/**
 * A failed save used to reach only `console.error` while the window closed
 * regardless, making a lost write indistinguishable from a successful one.
 */
describe("SettingsPage save failure", () => {
  it("keeps the window open and says what went wrong", async () => {
    saveRejects = true;
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    const save = await screen.findByRole("button", { name: /speichern/i });
    await user.click(save);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/nicht gespeichert/i);
    expect(alert).toHaveTextContent(/Zugriff verweigert/);

    expect(win.close).not.toHaveBeenCalled();
  });

  it("closes the window when the save succeeds", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: /speichern/i }));

    await waitFor(() => expect(win.close).toHaveBeenCalled());
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

/**
 * The log directory is not a user choice: the log plugin's target is fixed
 * before the app handle exists. It was an editable field that nothing read,
 * naming a directory that did not exist on macOS.
 */
describe("SettingsPage log directory", () => {
  it("shows the path read-only", async () => {
    render(<SettingsPage isDev={false} />);

    const field = await screen.findByLabelText(/protokollverzeichnis/i);
    expect(field).toHaveValue(LOG_DIR);
    expect(field).toHaveAttribute("readonly");
  });

  it("opens the directory in the file browser", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: /anzeigen/i }));

    expect(vi.mocked(revealItemInDir)).toHaveBeenCalledWith(LOG_DIR);
  });
});
