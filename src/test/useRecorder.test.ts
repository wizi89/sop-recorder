import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useRecorder } from "../hooks/useRecorder";

const mockInvoke = vi.mocked(invoke);

describe("useRecorder", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("starts in idle state", () => {
    const { result } = renderHook(() => useRecorder());
    expect(result.current.status).toBe("idle");
    expect(result.current.outputDir).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.statusMessage).toBe("");
  });

  it("transitions to recording on start", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);
    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });

    expect(result.current.status).toBe("recording");
    expect(mockInvoke).toHaveBeenCalledWith("start_recording");
  });

  it("transitions to processing then done on stop", async () => {
    const outputDir = "C:\\Users\\test\\output";
    mockInvoke
      .mockResolvedValueOnce(undefined) // start_recording
      .mockResolvedValueOnce(outputDir) // stop_recording
      .mockResolvedValueOnce(undefined); // run_generation

    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });

    await act(async () => {
      await result.current.stop();
    });

    expect(result.current.status).toBe("done");
    expect(result.current.outputDir).toBe(outputDir);
    expect(result.current.statusMessage).toBe("");
    expect(mockInvoke).toHaveBeenCalledWith("stop_recording");
    expect(mockInvoke).toHaveBeenCalledWith("run_generation", {
      outputDir,
    });
  });

  it("transitions to error on start failure", async () => {
    mockInvoke.mockRejectedValueOnce(new Error("No mic"));
    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });

    expect(result.current.status).toBe("error");
    expect(result.current.error).toContain("No mic");
  });

  it("transitions to error on generation failure", async () => {
    mockInvoke
      .mockResolvedValueOnce(undefined) // start
      .mockResolvedValueOnce("C:\\out") // stop
      .mockRejectedValueOnce(new Error("Upload failed")); // generate

    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      await result.current.stop();
    });

    expect(result.current.status).toBe("error");
    expect(result.current.error).toContain("Upload failed");
  });

  it("updates status message via setStatusMessage", async () => {
    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      result.current.setStatusMessage("Uploading...");
    });

    expect(result.current.statusMessage).toBe("Uploading...");
  });
});
