import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { ErrorBoundary } from "../components/ErrorBoundary";
import type { ErrorReport } from "../lib/tauri";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve("0.15.0")),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    label: "main",
    onFocusChanged: vi.fn(() => Promise.resolve(() => {})),
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

function report(overrides: Partial<ErrorReport> = {}): ErrorReport {
  return {
    schema_version: 1,
    report_id: "abcdef01-2222-3333-4444-555555555555",
    kind: "command_error",
    occurred_at: "2026-09-03T12:00:00+00:00",
    app_version: "0.15.0",
    os: "macos",
    os_version: "15.6",
    arch: "aarch64",
    locale: "de_DE",
    phase: "processing",
    message: "Upload failed: 500",
    location: null,
    log_tail: [],
    settings: null,
    job_id: null,
    comment: null,
    consent: "pending",
    ...overrides,
  };
}

function Boom(): never {
  throw new Error("Anzeige der Schritte fehlgeschlagen");
}

describe("ErrorBoundary (design D6)", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(null);
    // React logs the caught error; the test asserts on the report, not stderr.
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("creates one ui_error report and keeps something on screen", async () => {
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      const calls = vi
        .mocked(invoke)
        .mock.calls.filter(([cmd]) => cmd === "create_error_report");
      expect(calls).toHaveLength(1);
      expect(calls[0][1]).toMatchObject({ kind: "ui_error" });
      expect(String((calls[0][1] as { message: string }).message)).toContain(
        "Anzeige der Schritte fehlgeschlagen",
      );
    });

    // The window does not go blank.
    expect(
      screen.getByText(/Anzeige der Schritte fehlgeschlagen/),
    ).toBeInTheDocument();
  });
});

describe("mode always (design D1)", () => {
  let stored: ErrorReport[] = [];

  beforeEach(() => {
    stored = [report()];
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
      const a = (args ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "refresh_session":
          return { logged_in: true, email: "anna@example.de" };
        case "get_settings":
          return {
            output_dir: "",
            logs_dir: "",
            hide_from_screenshots: true,
            api_key: null,
            upload_target: null,
            skip_pii_check: false,
            pipeline_version: 1,
            generation_model: "azure/gpt-4.1",
            error_reports: "always",
          };
        case "get_quota":
          return {
            count: 1,
            limit: 10,
            remaining: 9,
            features: { advanced_settings: false },
          };
        case "get_microphone_permission_state":
        case "get_screen_recording_permission_state":
        case "get_accessibility_permission_state":
          return "granted";
        case "list_error_reports":
          return stored.map((r) => ({ ...r }));
        case "decide_error_report": {
          const id = a.reportId as string;
          stored = stored.map((r) =>
            r.report_id === id ? { ...r, consent: "granted" as const } : r,
          );
          return stored.find((r) => r.report_id === id) ?? null;
        }
        case "submit_error_reports": {
          const granted = stored.filter((r) => r.consent === "granted");
          stored = stored.filter((r) => r.consent !== "granted");
          return granted.map((r) => ({ report_id: r.report_id, number: "abcdef01" }));
        }
        default:
          return null;
      }
    });
  });

  it("sends without a dialog and puts the number in the status bar", async () => {
    const { default: App } = await import("../App");
    render(<App />);

    await waitFor(() =>
      expect(
        screen.getByText("Fehlerbericht gesendet -- Nummer abcdef01"),
      ).toBeInTheDocument(),
    );

    // No dialog: neither the consent buttons nor the confirmation appear.
    expect(screen.queryByRole("button", { name: "Bericht senden" })).toBeNull();
    expect(screen.queryByText(/Berichtsnummer:/)).toBeNull();
  });
});
