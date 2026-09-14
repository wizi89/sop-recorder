import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPage } from "../components/SettingsPage";

/// Leaving the beta channel does not move the installation back down -- the
/// updater only ever offers a higher version -- so the settings page says how
/// long the user stays put. Silence there reads as a broken updater, which is
/// the support ticket this note exists to prevent.

let mockVersion = "0.19.0-rc.1";
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve(mockVersion)),
}));

let mockBetaUpdates = false;

const mockSettings = () => ({
  output_dir: "C:\docs\workflows",
  logs_dir: "C:\data\logs",
  hide_from_screenshots: true,
  upload_target: null,
  skip_pii_check: false,
  error_reports: "ask",
  beta_updates: mockBetaUpdates,
});

beforeEach(() => {
  mockVersion = "0.19.0-rc.1";
  mockBetaUpdates = false;
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_settings") return mockSettings();
    if (cmd === "get_quota")
      return {
        count: 0,
        limit: 100,
        remaining: 100,
        features: { advanced_settings: false },
        generation_settings: {
          pipeline_versions: [1],
          models: ["azure/gpt-4.1"],
          default_model: "azure/gpt-4.1",
        },
      };
    if (cmd === "are_error_reports_forced_off") return false;
    return;
  });
});

const note = () => screen.queryByText(/bis 0\.19\.0 erscheint/i);

describe("the beta channel note", () => {
  it("names the stable version a prerelease build is waiting for", async () => {
    render(<SettingsPage isDev={false} />);
    await waitFor(() => expect(note()).toBeInTheDocument());
    expect(note()).toHaveTextContent("0.19.0-rc.1");
  });

  it("stays hidden while the user is still on the beta channel", async () => {
    mockBetaUpdates = true;
    render(<SettingsPage isDev={false} />);
    await waitFor(() => expect(screen.getByText(/pii/i)).toBeInTheDocument());
    expect(note()).not.toBeInTheDocument();
  });

  it("stays hidden on a stable build, which has nothing to wait for", async () => {
    mockVersion = "0.19.0";
    render(<SettingsPage isDev={false} />);
    await waitFor(() => expect(screen.getByText(/pii/i)).toBeInTheDocument());
    expect(note()).not.toBeInTheDocument();
  });
});
