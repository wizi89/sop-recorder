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
  | "error";

interface RecorderState {
  status: RecorderStatus;
  statusMessage: string;
  outputDir: string | null;
  error: string | null;
}

export function useRecorder() {
  const [state, setState] = useState<RecorderState>({
    status: "idle",
    statusMessage: "",
    outputDir: null,
    error: null,
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
      });
    } catch (e) {
      setState({
        status: "error",
        statusMessage: "",
        outputDir: null,
        error: String(e),
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
      });
      generatingRef.current = true;
      await runGeneration(outputDir);
      setState((s) => ({
        ...s,
        status: "done",
        statusMessage: "",
      }));
    } catch (e) {
      setState((s) => ({
        ...s,
        status: "error",
        error: String(e),
      }));
    } finally {
      generatingRef.current = false;
    }
  }, []);

  const reset = useCallback(() => {
    setState({
      status: "idle",
      statusMessage: "",
      outputDir: null,
      error: null,
    });
  }, []);

  const setStatusMessage = useCallback((msg: string) => {
    setState((s) => ({ ...s, statusMessage: msg }));
  }, []);

  const setProcessing = useCallback(() => {
    setState((s) => ({ ...s, status: "processing" as const, error: null, statusMessage: "" }));
  }, []);

  return { ...state, start, stop, reset, setStatusMessage, setProcessing };
}
