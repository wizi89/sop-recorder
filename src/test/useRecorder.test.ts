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

  it("stop transitions to review, then confirmGeneration completes to done", async () => {
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

    // Stop now transitions to review, NOT directly to processing/done
    expect(result.current.status).toBe("review");
    expect(result.current.outputDir).toBe(outputDir);

    await act(async () => {
      await result.current.confirmGeneration();
    });

    expect(result.current.status).toBe("done");
    expect(result.current.outputDir).toBe(outputDir);
    expect(result.current.statusMessage).toBe("");
    expect(mockInvoke).toHaveBeenCalledWith("stop_recording");
    expect(mockInvoke).toHaveBeenCalledWith("run_generation", {
      outputDir,
    });
  });

  it("cancelFromReview discards session and returns to idle", async () => {
    const outputDir = "C:\\Users\\test\\output";
    mockInvoke
      .mockResolvedValueOnce(undefined) // start_recording
      .mockResolvedValueOnce(outputDir); // stop_recording

    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      await result.current.stop();
    });
    expect(result.current.status).toBe("review");

    await act(async () => {
      result.current.cancelFromReview();
    });

    expect(result.current.status).toBe("idle");
    expect(result.current.outputDir).toBeNull();
    // run_generation was NOT called
    expect(mockInvoke).not.toHaveBeenCalledWith("run_generation", expect.anything());
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

  it("transitions to error on generation failure after review confirm", async () => {
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
    await act(async () => {
      await result.current.confirmGeneration();
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

  it("routes rate_limit errors to rate_limited status after review confirm", async () => {
    mockInvoke
      .mockResolvedValueOnce(undefined) // start
      .mockResolvedValueOnce("C:\\out") // stop
      .mockRejectedValueOnce(
        'Server error (429 Too Many Requests): {"error":"rate_limit","message":"Generation limit reached (10/10). Upgrade your plan for higher limits."}',
      );

    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      await result.current.stop();
    });
    await act(async () => {
      await result.current.confirmGeneration();
    });

    expect(result.current.status).toBe("rate_limited");
    expect(result.current.error).toBeNull();
    expect(result.current.rateLimit).toEqual({ count: 10, limit: 10 });
  });

  it("setError routes rate_limit messages to rate_limited status", async () => {
    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      result.current.setError(
        '{"error":"rate_limit","message":"Generation limit reached (7/10). Upgrade your plan for higher limits."}',
      );
    });

    expect(result.current.status).toBe("rate_limited");
    expect(result.current.rateLimit).toEqual({ count: 7, limit: 10 });
  });

  it("setError falls through to error status for non-rate-limit errors", async () => {
    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      result.current.setError("Something else went wrong");
    });

    expect(result.current.status).toBe("error");
    expect(result.current.error).toBe("Something else went wrong");
  });

  it("error path preserves outputDir for retry-from-disk", async () => {
    const outputDir = "C:\\Users\\test\\output";
    mockInvoke
      .mockResolvedValueOnce(undefined) // start
      .mockResolvedValueOnce(outputDir) // stop
      .mockRejectedValueOnce(new Error("Upload failed")); // generate

    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      await result.current.stop();
    });
    await act(async () => {
      await result.current.confirmGeneration();
    });

    expect(result.current.status).toBe("error");
    expect(result.current.outputDir).toBe(outputDir); // preserved
  });

  it("setRateLimited transitions directly to rate_limited with provided numbers", async () => {
    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      result.current.setRateLimited(10, 10);
    });

    expect(result.current.status).toBe("rate_limited");
    expect(result.current.rateLimit).toEqual({ count: 10, limit: 10 });
    expect(result.current.error).toBeNull();
  });

  it("dismissRateLimit returns to idle while preserving outputDir", async () => {
    const outputDir = "C:\\Users\\test\\output";
    mockInvoke
      .mockResolvedValueOnce(undefined) // start
      .mockResolvedValueOnce(outputDir) // stop
      .mockRejectedValueOnce(
        '{"error":"rate_limit","message":"Generation limit reached (5/5)."}',
      );

    const { result } = renderHook(() => useRecorder());

    await act(async () => {
      await result.current.start();
    });
    await act(async () => {
      await result.current.stop();
    });
    // Review -> confirm triggers the generation which fails with rate_limit
    await act(async () => {
      await result.current.confirmGeneration();
    });

    // Preconditions: rate_limited state with outputDir preserved
    expect(result.current.status).toBe("rate_limited");
    expect(result.current.outputDir).toBe(outputDir);
    expect(result.current.rateLimit).toEqual({ count: 5, limit: 5 });

    // Dismiss the rate-limit modal
    await act(async () => {
      result.current.dismissRateLimit();
    });

    expect(result.current.status).toBe("idle");
    expect(result.current.rateLimit).toBeNull();
    expect(result.current.outputDir).toBe(outputDir); // still preserved for retry-from-disk
  });
});
