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
/** The Rust-side mode. `never` hides the queue and refuses to create. */
let backendMode: "ask" | "always" | "never" = "ask";
/** Whether the Rust side has a session. Without one `submit_error_reports`
 *  sends nothing and the granted files stay on disk -- the fake used to send
 *  regardless, which made the signed-out case pass for the wrong reason. */
let backendSignedIn = true;
let nextId = 0;

beforeEach(() => {
  stored = [];
  submitted = [];
  backendMode = "ask";
  backendSignedIn = true;
  nextId = 0;
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "list_error_reports") {
      // commands/error_reports.rs: mode `never` answers with an empty list
      // rather than revealing what is on disk.
      return backendMode === "never" ? [] : stored.map((r) => ({ ...r }));
    }
    if (cmd === "create_error_report") {
      if (backendMode === "never") return null;
      const created = report({
        report_id: `0000000${++nextId}-2222-3333-4444-555555555555`,
        kind: a.kind as ErrorReport["kind"],
        phase: a.phase as string,
        message: a.message as string,
        job_id: (a.jobId as string) ?? null,
      });
      stored.push(created);
      return { ...created };
    }
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
      if (!backendSignedIn) return [];
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

  // -- The workflow, end to end over the queue ------------------------------
  //
  // These exist because three defects in a row were found by hand rather than
  // here: a decline that appeared to come back, a backlog mistaken for one
  // report returning, and a mode that silenced everything. Each is a property
  // of the queue across a restart, which no single-call mock can express.

  it("a declined report is gone, and stays gone across a restart", async () => {
    stored = [report({ report_id: "aaaaaaaa-0000-0000-0000-000000000000" })];

    const first = renderHook(() => useErrorReports({ loggedIn: true, mode: "ask" }));
    await waitFor(() => expect(first.result.current.current).not.toBeNull());
    await first.result.current.decline(first.result.current.current!);
    await waitFor(() => expect(first.result.current.current).toBeNull());

    // A restart is a fresh mount with no in-memory record of what was answered.
    // Only the deletion on disk can keep the dialog shut, which is the whole
    // point: declining must be final, not remembered.
    first.unmount();
    const afterRestart = renderHook(() => useErrorReports({ loggedIn: true, mode: "ask" }));
    await waitFor(() => expect(afterRestart.result.current.pending).toEqual([]));
    expect(afterRestart.result.current.current).toBeNull();
    expect(submitted).toEqual([]);
  });

  it("works through a backlog one report at a time", async () => {
    // Answering one report reveals the next, which looks exactly like the same
    // dialog coming back if you are not watching the ids.
    stored = [
      report({ report_id: "aaaaaaaa-0000-0000-0000-000000000000" }),
      report({ report_id: "bbbbbbbb-0000-0000-0000-000000000000" }),
      report({ report_id: "cccccccc-0000-0000-0000-000000000000" }),
    ];

    const { result } = renderHook(() => useErrorReports({ loggedIn: true, mode: "ask" }));
    await waitFor(() => expect(result.current.pending).toHaveLength(3));

    const seen: string[] = [];
    for (let i = 0; i < 3; i++) {
      // Wait for a report that is not the one just answered. Reading
      // `current` straight after a decline can still see the previous value,
      // which would decline the same report twice and leave one behind.
      await waitFor(() => {
        const shown = result.current.current;
        expect(shown).not.toBeNull();
        expect(seen).not.toContain(shown!.report_id);
      });
      const shown = result.current.current!;
      seen.push(shown.report_id);
      await result.current.decline(shown);
    }

    await waitFor(() => expect(result.current.current).toBeNull());
    expect(new Set(seen).size, "each report is asked about once").toBe(3);
    expect(stored).toEqual([]);
  });

  it("creates a report and puts it on screen", async () => {
    const { result } = renderHook(() => useErrorReports({ loggedIn: true, mode: "ask" }));
    await waitFor(() => expect(result.current.pending).toEqual([]));

    await result.current.create("command_error", "settings", "Upload fehlgeschlagen: 500");

    await waitFor(() => expect(result.current.current).not.toBeNull());
    expect(result.current.current!.message).toBe("Upload fehlgeschlagen: 500");
    expect(result.current.current!.phase).toBe("settings");
  });

  it("mode never creates nothing and hides what is already queued", async () => {
    // The queue is not deleted, only withheld -- switching back must bring it
    // into view rather than having lost anything.
    stored = [report({ report_id: "aaaaaaaa-0000-0000-0000-000000000000" })];
    backendMode = "never";

    const off = renderHook(() => useErrorReports({ loggedIn: true, mode: "never" }));
    await waitFor(() => expect(off.result.current.pending).toEqual([]));
    expect(await off.result.current.create("ui_error", "settings", "kaputt")).toBeNull();
    expect(off.result.current.current).toBeNull();
    off.unmount();

    backendMode = "ask";
    const on = renderHook(() => useErrorReports({ loggedIn: true, mode: "ask" }));
    await waitFor(() => expect(on.result.current.current).not.toBeNull());
  });

  it("a granted report that could not be sent is retried after the next sign-in", async () => {
    // The signed-out case and the failed-send case are the same case: consent
    // is on disk and the file is still there.
    stored = [report({ report_id: "aaaaaaaa-0000-0000-0000-000000000000" })];

    backendSignedIn = false;
    const out = renderHook(() => useErrorReports({ loggedIn: false, mode: "ask" }));
    await waitFor(() => expect(out.result.current.current).not.toBeNull());
    await out.result.current.grant(out.result.current.current!, "beim Speichern");
    expect(stored[0].consent).toBe("granted");
    expect(submitted, "nothing leaves without a session").toEqual([]);
    out.unmount();

    backendSignedIn = true;
    const back = renderHook(() => useErrorReports({ loggedIn: true, mode: "ask" }));
    await waitFor(() => expect(submitted).toEqual(["aaaaaaaa-0000-0000-0000-000000000000"]));
    expect(back.result.current.current).toBeNull();
  });
});
