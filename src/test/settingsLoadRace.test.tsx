import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPage } from "../components/SettingsPage";

/**
 * The "first save does not stick" report from the 2026-09-03 macOS test.
 *
 * `getSettings().then(setSettings)` replaced the *entire* form state when it
 * resolved, so anything the user changed before then was reverted with no
 * visible cue, and the save wrote the old value. The window was slow enough for
 * that to be reachable because `get_settings` read the macOS keychain, which
 * after a reinstall prompts or blocks for seconds.
 *
 * These tests hold the load open deliberately, which is the only way to make
 * the race deterministic.
 */
describe("SettingsPage load race", () => {
  const stored = {
    output_dir: "C:\\docs\\workflows",
    logs_dir: "/Users/m/Library/Logs/com.cogniclone.recorder",
    hide_from_screenshots: true,
    upload_target: null,
    skip_pii_check: false,
    pipeline_version: 1,
    generation_model: "azure/gpt-4.1",
    error_reports: "ask",
  };

  let resolveSettings: (value: unknown) => void;
  let saved: unknown[] = [];

  beforeEach(() => {
    saved = [];
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "get_settings") {
        return new Promise((resolve) => {
          resolveSettings = resolve;
        });
      }
      if (cmd === "save_settings") {
        saved.push((args as { settings: unknown }).settings);
        return;
      }
      if (cmd === "get_quota") {
        return { count: 0, limit: 100, remaining: 100, features: { advanced_settings: false } };
      }
      if (cmd === "are_error_reports_forced_off") return false;
      return;
    });
  });

  it("disables the controls until the stored settings have arrived", async () => {
    render(<SettingsPage isDev={false} />);

    const save = await screen.findByRole("button", { name: /wird geladen/i });
    expect(save).toBeDisabled();

    await act(async () => {
      resolveSettings({ ...stored });
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /speichern/i })).toBeEnabled(),
    );
  });

  /// The exact sequence from the report: change a setting, then let the load
  /// land. Previously the click was accepted, the modal opened, the toggle went
  /// on -- and the arriving load turned it off again with nothing on screen to
  /// say so. Now the click cannot happen at all, so the form never shows a
  /// state that disagrees with what a save would write.
  it("refuses an edit while the load is pending, rather than accepting and reverting it", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    const piiToggle = () =>
      screen.getAllByRole("button").filter((b) => b.getAttribute("data-checked") !== null)[1];

    await user.click(piiToggle());

    // On the previous code this modal opened and the toggle went on.
    expect(screen.queryByText(/pii/i)).toBeInTheDocument();
    expect(piiToggle()).toBeDisabled();
    expect(piiToggle()).toHaveAttribute("data-checked", "false");

    await act(async () => {
      resolveSettings({ ...stored });
    });

    // And the state after the load is the stored one, with no edit lost in
    // between, because none was ever accepted.
    await waitFor(() => expect(piiToggle()).toBeEnabled());
    expect(piiToggle()).toHaveAttribute("data-checked", "false");
  });

  it("saves what the form shows once the load has landed", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await act(async () => {
      resolveSettings({ ...stored });
    });
    const save = await screen.findByRole("button", { name: /speichern/i });
    const piiToggle = () =>
      screen.getAllByRole("button").filter((b) => b.getAttribute("data-checked") !== null)[1];

    await user.click(piiToggle());
    await user.click(
      await screen.findByRole("button", { name: /verstanden|fortfahren|akzeptier/i }),
    );
    await waitFor(() => expect(piiToggle()).toHaveAttribute("data-checked", "true"));

    await user.click(save);

    await waitFor(() => expect(saved).toHaveLength(1));
    expect((saved[0] as { skip_pii_check: boolean }).skip_pii_check).toBe(true);
  });
});
