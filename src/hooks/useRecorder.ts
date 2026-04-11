import { useState, useCallback, useEffect, useRef } from "react";
import {
  startRecording,
  stopRecording,
  runGeneration,
} from "../lib/tauri";
import { t } from "../i18n";
import { parseRateLimit, type RateLimitInfo } from "../lib/serverErrors";

export type RecorderStatus =
  | "idle"
  | "recording"
  | "review"
  | "processing"
  | "done"
  | "error"
  | "pii_blocked"
  | "rate_limited";

interface RecorderState {
  status: RecorderStatus;
  statusMessage: string;
  outputDir: string | null;
  error: string | null;
  piiFindings: unknown | null;
  rateLimit: RateLimitInfo | null;
}

export function useRecorder() {
  const [state, setState] = useState<RecorderState>({
    status: "idle",
    statusMessage: "",
    outputDir: null,
    error: null,
    piiFindings: null,
    rateLimit: null,
  });
  const generatingRef = useRef(false);
  // Keep the latest outputDir in a ref so async actions can read it without
  // falling back into stale-closure bugs across act() boundaries in tests.
  const outputDirRef = useRef<string | null>(null);
  useEffect(() => {
    outputDirRef.current = state.outputDir;
  }, [state.outputDir]);

  const start = useCallback(async () => {
    try {
      await startRecording();
      setState({
        status: "recording",
        statusMessage: "",
        outputDir: null,
        error: null,
        piiFindings: null,
        rateLimit: null,
      });
    } catch (e) {
      setState({
        status: "error",
        statusMessage: "",
        outputDir: null,
        error: String(e),
        piiFindings: null,
        rateLimit: null,
      });
    }
  }, []);

  /**
   * Stop the recording session and transition to `review` so the user can
   * inspect captured screenshots before committing to a generation.
   * Does NOT invoke `runGeneration` -- that only happens when the user
   * confirms from the review screen via `confirmGeneration`.
   *
   * While `stopRecording` runs on the Rust side (which now waits for any
   * in-flight screenshot captures to finish writing) we transition to
   * `processing` + `stopping` status so the compact bar disappears and the
   * user sees a full-size "Aufnahme wird verarbeitet..." message. Only
   * after Rust returns do we flip to `review` with a stable filesystem.
   *
   * If zero screenshots were captured we skip review and go straight back
   * to idle with a `no_clicks` hint, matching the legacy behavior.
   */
  const stop = useCallback(async () => {
    if (generatingRef.current) return;
    // Immediately show the localized "stopping" message so the user gets
    // feedback while the Rust side waits for in-flight captures. Uses the
    // already-translated string directly (no sentinel + mapping dance) so
    // there is no window where a raw English identifier can leak to the UI.
    setState((s) => ({
      ...s,
      status: "processing" as const,
      statusMessage: t("status.stopping"),
      error: null,
      piiFindings: null,
      rateLimit: null,
    }));
    try {
      const outputDir = await stopRecording();
      setState((s) => ({
        ...s,
        status: "review",
        outputDir,
        statusMessage: "",
        error: null,
        piiFindings: null,
        rateLimit: null,
      }));
    } catch (e) {
      const msg = String(e);
      setState((s) => {
        if (s.status === "pii_blocked") return s;
        if (msg.includes("No screenshots found")) {
          return { ...s, status: "idle" as const, error: null, statusMessage: "no_clicks" };
        }
        return { ...s, status: "error" as const, error: msg };
      });
    }
  }, []);

  /**
   * Confirm from the review screen: invoke the generation pipeline against
   * the recording session directory and handle the post-generation outcomes
   * (done, error, pii_blocked, rate_limited) via the same logic the old
   * `stop()` used to run inline.
   *
   * Reads outputDir via a ref rather than via closure so the latest value
   * is visible even across the async boundary.
   */
  const confirmGeneration = useCallback(async () => {
    if (generatingRef.current) return;
    const currentOutputDir = outputDirRef.current;
    if (!currentOutputDir) return;
    setState((s) => ({ ...s, status: "processing" as const, statusMessage: "", error: null }));
    generatingRef.current = true;
    try {
      await runGeneration(currentOutputDir);
      setState((s) => ({ ...s, status: "done" as const, statusMessage: "" }));
    } catch (e) {
      const msg = String(e);
      setState((s) => {
        if (s.status === "pii_blocked") return s;
        const rl = parseRateLimit(msg);
        if (rl) {
          return { ...s, status: "rate_limited" as const, error: null, rateLimit: rl };
        }
        return { ...s, status: "error" as const, error: msg };
      });
    } finally {
      generatingRef.current = false;
    }
  }, []);

  /**
   * Cancel from the review screen: discard the captured session and return
   * to idle. The recording dir is left on disk (future cleanup tracked in
   * memory: future_cancel_cleanup.md).
   */
  const cancelFromReview = useCallback(() => {
    setState({
      status: "idle",
      statusMessage: "",
      outputDir: null,
      error: null,
      piiFindings: null,
      rateLimit: null,
    });
  }, []);

  const cancel = useCallback(async () => {
    try {
      await stopRecording();
    } catch {
      // ignore -- best effort cleanup
    }
    setState({
      status: "idle",
      statusMessage: "",
      outputDir: null,
      error: null,
      piiFindings: null,
      rateLimit: null,
    });
  }, []);

  const reset = useCallback(() => {
    setState({
      status: "idle",
      statusMessage: "",
      outputDir: null,
      error: null,
      piiFindings: null,
      rateLimit: null,
    });
  }, []);

  const setStatusMessage = useCallback((msg: string) => {
    setState((s) => ({ ...s, statusMessage: msg }));
  }, []);

  const setProcessing = useCallback(() => {
    setState((s) => ({ ...s, status: "processing" as const, error: null, statusMessage: "" }));
  }, []);

  const setError = useCallback((msg: string) => {
    // Route rate-limit errors to the structured modal even when they arrive
    // via the SSE `error` event path instead of the thrown-error path.
    const rl = parseRateLimit(msg);
    if (rl) {
      setState((s) => ({ ...s, status: "rate_limited" as const, error: null, rateLimit: rl }));
      return;
    }
    setState((s) => ({ ...s, status: "error" as const, error: msg }));
  }, []);

  const setPiiBlocked = useCallback((findings: unknown) => {
    setState((s) => ({ ...s, status: "pii_blocked" as const, piiFindings: findings }));
  }, []);

  /**
   * Put the recorder directly into the rate-limited state with explicit
   * count/limit numbers. Used to block a recording attempt BEFORE any capture
   * happens when we already know the user is at quota.
   */
  const setRateLimited = useCallback((count: number, limit: number) => {
    setState((s) => ({
      ...s,
      status: "rate_limited" as const,
      error: null,
      rateLimit: { count, limit },
    }));
  }, []);

  /**
   * Dismiss the rate-limit modal while preserving the recording session
   * directory so the user can still retry-from-disk after topping up quota.
   * Transitions to idle + clears rateLimit but keeps outputDir intact.
   */
  const dismissRateLimit = useCallback(() => {
    setState((s) => ({
      ...s,
      status: "idle" as const,
      error: null,
      rateLimit: null,
      // deliberately keep outputDir so the retry-from-disk action can reuse it
    }));
  }, []);

  return {
    ...state,
    start,
    stop,
    confirmGeneration,
    cancelFromReview,
    cancel,
    reset,
    setStatusMessage,
    setProcessing,
    setError,
    setPiiBlocked,
    setRateLimited,
    dismissRateLimit,
  };
}
