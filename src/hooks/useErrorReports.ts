import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ERROR_REPORT_CREATED,
  createErrorReport,
  decideErrorReport,
  listErrorReports,
  submitErrorReports,
  type ErrorReport,
  type ErrorReportMode,
  type SubmittedReport,
} from "../lib/tauri";

/**
 * The webview's half of error reporting (design D1, D5, D7).
 *
 * The Rust side owns every report: it writes one to disk before anyone sees
 * it, scrubs it there, and keeps it until a decision is made. This hook only
 * decides what is on screen and when a submission runs.
 *
 * It lists pending reports on mount, which is how a crash from a previous run
 * surfaces -- the panic hook wrote the file and did nothing else, so the next
 * launch is the first moment anyone can be asked. It also listens for
 * `error_report:created`, which a panic on a thread other than the main one
 * emits while the process is still running.
 *
 * Submission runs whenever a session exists: right after consent when signed
 * in, and after the next successful sign-in otherwise. That one mechanism is
 * also the retry, so there is no second queue.
 */

export interface UseErrorReportsOptions {
  /** Whether a session exists. A flip to true triggers a submission. */
  loggedIn: boolean;
  /** The saved mode. Under `always` a report is sent with no dialog. */
  mode: ErrorReportMode;
  /** Shown in the status bar after an automatic send (design D1). */
  onAutoSent?: (number: string) => void;
}

export function useErrorReports({
  loggedIn,
  mode,
  onAutoSent,
}: UseErrorReportsOptions) {
  const [pending, setPending] = useState<ErrorReport[]>([]);
  const [sent, setSent] = useState<SubmittedReport | null>(null);
  // Reports the user has already answered this session. A decline deletes the
  // file, so this only guards the window between the answer and the refresh.
  const answered = useRef<Set<string>>(new Set());
  const onAutoSentRef = useRef(onAutoSent);
  useEffect(() => {
    onAutoSentRef.current = onAutoSent;
  }, [onAutoSent]);
  const modeRef = useRef(mode);
  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);

  const refresh = useCallback(async () => {
    try {
      const reports = await listErrorReports();
      setPending(reports.filter((r) => !answered.current.has(r.report_id)));
      return reports;
    } catch {
      return [];
    }
  }, []);

  const submit = useCallback(async () => {
    try {
      return await submitErrorReports();
    } catch {
      return [];
    }
  }, []);

  /**
   * Send without asking, for mode `always`. Consent was given once, in the
   * settings or on the checkbox, and re-asking twenty times a day is what
   * makes a user turn the feature off entirely.
   */
  const grantAndSend = useCallback(
    async (report: ErrorReport, comment?: string | null) => {
      answered.current.add(report.report_id);
      await decideErrorReport(report.report_id, true, comment ?? null);
      const results = await submit();
      const mine = results.find((r) => r.report_id === report.report_id) ?? null;
      await refresh();
      return mine;
    },
    [refresh, submit],
  );

  const decline = useCallback(
    async (report: ErrorReport) => {
      answered.current.add(report.report_id);
      await decideErrorReport(report.report_id, false, null);
      await refresh();
    },
    [refresh],
  );

  const grant = useCallback(
    async (report: ErrorReport, comment?: string | null) => {
      const result = await grantAndSend(report, comment);
      setSent(result);
      return result;
    },
    [grantAndSend],
  );

  /** Create a report for a failure the webview saw and put it on screen. */
  const create = useCallback(
    async (
      kind: "command_error" | "ui_error",
      phase: string,
      message: string,
      jobId?: string | null,
    ) => {
      const report = await createErrorReport(kind, phase, message, jobId);
      if (!report) return null;
      await refresh();
      return report;
    },
    [refresh],
  );

  const dismissSent = useCallback(() => setSent(null), []);

  useEffect(() => {
    void refresh();
    const unlisten = listen(ERROR_REPORT_CREATED, () => void refresh());
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [refresh]);

  // A report created while signed out waits with its decision recorded, and a
  // submission that failed is simply still granted. Both are served by
  // sending everything granted the moment a session exists.
  useEffect(() => {
    if (!loggedIn) return;
    void submit().then((results) => {
      if (results.length > 0) void refresh();
    });
  }, [loggedIn, submit, refresh]);

  // Mode `always`: no dialog, a notice with the number instead.
  useEffect(() => {
    if (modeRef.current !== "always") return;
    const next = pending.find((r) => r.consent === "pending");
    if (!next) return;
    void grantAndSend(next).then((result) => {
      if (result) onAutoSentRef.current?.(result.number);
    });
  }, [pending, grantAndSend]);

  // Under `always` the dialog never opens; under `never` nothing is listed.
  const current = mode === "always" ? null : (pending[0] ?? null);

  return { pending, current, sent, create, grant, decline, refresh, dismissSent };
}
