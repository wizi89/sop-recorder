import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  errorReportPhase,
  setErrorReportPhase,
  createErrorReport,
} from "../lib/tauri";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = vi.mocked(invoke);

describe("publishing the report phase", () => {
  beforeEach(() => {
    invoked.mockReset();
    invoked.mockResolvedValue(undefined);
  });

  it("sends the phase to the Rust side and remembers it", async () => {
    await setErrorReportPhase("recording");

    expect(invoked).toHaveBeenCalledWith("set_error_report_phase", { phase: "recording" });
    expect(errorReportPhase()).toBe("recording");
  });

  it("does not repeat a phase it has already published", async () => {
    await setErrorReportPhase("review");
    invoked.mockClear();
    await setErrorReportPhase("review");

    expect(invoked).not.toHaveBeenCalled();
  });

  it("republishes when forced, for reclaiming focus from the settings window", async () => {
    await setErrorReportPhase("idle");
    invoked.mockClear();
    await setErrorReportPhase("idle", true);

    expect(invoked).toHaveBeenCalledWith("set_error_report_phase", { phase: "idle" });
  });

  it("keeps the mirror when the command fails, and does not throw", async () => {
    await setErrorReportPhase("processing");
    invoked.mockRejectedValueOnce(new Error("kein Backend"));

    await expect(setErrorReportPhase("login")).resolves.toBeUndefined();
    // A phase that did not land mislabels a report; it must never lose one.
    expect(errorReportPhase()).toBe("login");
  });

  it("a report raised outside React carries the live phase, not a literal", async () => {
    // This is the defect the whole file exists for: `main.tsx` and the error
    // boundary are outside the component tree and cannot read React state, so
    // they used to pass the string "unknown" and every webview error arrived
    // claiming the user was doing nothing in particular.
    await setErrorReportPhase("settings");
    invoked.mockClear();
    invoked.mockResolvedValue(null);

    await createErrorReport("ui_error", errorReportPhase(), "kaputt");

    expect(invoked).toHaveBeenCalledWith(
      "create_error_report",
      expect.objectContaining({ phase: "settings" }),
    );
  });
});
