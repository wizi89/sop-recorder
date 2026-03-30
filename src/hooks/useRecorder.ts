import { useState, useCallback, useRef } from "react";
import {
  startRecording,
  stopRecording,
  runGeneration,
} from "../lib/tauri";

export type RecorderStatus =
  | "idle"
  | "recording"
  | "processing"
  | "done"
  | "error"
  | "pii_blocked";

interface RecorderState {
  status: RecorderStatus;
  statusMessage: string;
  outputDir: string | null;
  error: string | null;
  piiFindings: unknown | null;
}

export function useRecorder() {
  const [state, setState] = useState<RecorderState>({
    status: "idle",
    statusMessage: "",
    outputDir: null,
    error: null,
    piiFindings: null,
  });
  const generatingRef = useRef(false);

  const start = useCallback(async () => {
    try {
      await startRecording();
      setState({
        status: "recording",
        statusMessage: "",
        outputDir: null,
        error: null,
        piiFindings: null,
      });
    } catch (e) {
      setState({
        status: "error",
        statusMessage: "",
        outputDir: null,
        error: String(e),
        piiFindings: null,
      });
    }
  }, []);

  const stop = useCallback(async () => {
    if (generatingRef.current) return;
    try {
      const outputDir = await stopRecording();
      setState({
        status: "processing",
        statusMessage: "",
        outputDir,
        error: null,
        piiFindings: null,
      });
      generatingRef.current = true;
      await runGeneration(outputDir);
      setState((s) => ({
        ...s,
        status: "done",
        statusMessage: "",
      }));
    } catch (e) {
      const msg = String(e);
      setState((s) => {
        // Don't override pii_blocked -- the SSE handler already set it
        if (s.status === "pii_blocked") return s;
        // No screenshots = nothing to process, go back to idle with a hint
        if (msg.includes("No screenshots found")) {
          return { ...s, status: "idle" as const, error: null, statusMessage: "no_clicks" };
        }
        return { ...s, status: "error" as const, error: msg };
      });
    } finally {
      generatingRef.current = false;
    }
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
    });
  }, []);

  const reset = useCallback(() => {
    setState({
      status: "idle",
      statusMessage: "",
      outputDir: null,
      error: null,
      piiFindings: null,
    });
  }, []);

  const setStatusMessage = useCallback((msg: string) => {
    setState((s) => ({ ...s, statusMessage: msg }));
  }, []);

  const setProcessing = useCallback(() => {
    setState((s) => ({ ...s, status: "processing" as const, error: null, statusMessage: "" }));
  }, []);

  const setError = useCallback((msg: string) => {
    setState((s) => ({ ...s, status: "error" as const, error: msg }));
  }, []);

  const setPiiBlocked = useCallback((findings: unknown) => {
    setState((s) => ({ ...s, status: "pii_blocked" as const, piiFindings: findings }));
  }, []);

  return { ...state, start, stop, cancel, reset, setStatusMessage, setProcessing, setError, setPiiBlocked };
}
