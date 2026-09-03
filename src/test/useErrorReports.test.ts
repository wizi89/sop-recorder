import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useErrorReports } from "../hooks/useErrorReports";
import type { ErrorReport } from "../lib/tauri";

function report(overrides: Partial<ErrorReport> = {}): ErrorReport {
  return {
    schema_version: 1,
    report_id: "11111111-2222-3333-4444-555555555555",
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
    log_tail: ["[INFO] Upload gestartet"],
    settings: null,
    job_id: null,
    comment: null,
    consent: "pending",
    ...overrides,
  };
}

let stored: ErrorReport[] = [];
let submitted: string[] = [];

beforeEach(() => {
  stored = [];
  submitted = [];
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "list_error_reports") return stored.map((r) => ({ ...r }));
    if (cmd === "decide_error_report") {
      const id = a.reportId as string;
      if (!a.grant) {
        stored = stored.filter((r) => r.report_id !== id);
        return null;
      }
      stored = stored.map((r) =>
        r.report_id === id
          ? { ...r, consent: "granted" as const, comment: (a.comment as string) ?? null }
          : r,
      );
      return stored.find((r) => r.report_id === id) ?? null;
    }
    if (cmd === "submit_error_reports") {
      const granted = stored.filter((r) => r.consent === "granted");
      submitted.push(...granted.map((r) => r.report_id));
      stored = stored.filter((r) => r.consent !== "granted");
      return granted.map((r) => ({
        report_id: r.report_id,
        number: r.report_id.replace(/-/g, "").slice(0, 8),
      }));
    }
    return null;
  });
});

describe("useErrorReports", () => {
  it("submits a granted report waiting from a signed-out session once signed in", async () => {
    // The login failure is the case this exists for: the report is created
    // and granted while there is no session to send it with (design D7).
    stored = [report({ consent: "granted", phase: "login" })];

    const { rerender } = renderHook(
      ({ loggedIn }) => useErrorReports({ loggedIn, mode: "ask" }),
      { initialProps: { loggedIn: false } },
    );

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("list_error_reports"));
    expect(submitted).toEqual([]);

    rerender({ loggedIn: true });
    await waitFor(() =>
      expect(submitted).toEqual(["11111111-2222-3333-4444-555555555555"]),
    );
  });

  it("puts a pending report on screen and clears it after a decline", async () => {
    stored = [report()];
    const { result } = renderHook(() =>
      useErrorReports({ loggedIn: true, mode: "ask" }),
    );

    await waitFor(() => expect(result.current.current).not.toBeNull());
    await result.current.decline(result.current.current!);

    await waitFor(() => expect(result.current.current).toBeNull());
    expect(submitted).toEqual([]);
    expect(stored).toEqual([]);
  });

  it("grants with the comment and reports the number back", async () => {
    stored = [report()];
    const { result } = renderHook(() =>
      useErrorReports({ loggedIn: true, mode: "ask" }),
    );

    await waitFor(() => expect(result.current.current).not.toBeNull());
    const sent = await result.current.grant(
      result.current.current!,
      "Ich habe auf Generieren geklickt",
    );

    expect(sent?.number).toBe("11111111");
    expect(invoke).toHaveBeenCalledWith("decide_error_report", {
      reportId: "11111111-2222-3333-4444-555555555555",
      grant: true,
      comment: "Ich habe auf Generieren geklickt",
    });
  });

  it("sends without a dialog under mode always", async () => {
    stored = [report()];
    const onAutoSent = vi.fn();
    const { result } = renderHook(() =>
      useErrorReports({ loggedIn: true, mode: "always", onAutoSent }),
    );

    await waitFor(() => expect(onAutoSent).toHaveBeenCalledWith("11111111"));
    // No dialog: `current` stays null throughout under `always`.
    expect(result.current.current).toBeNull();
  });
});
