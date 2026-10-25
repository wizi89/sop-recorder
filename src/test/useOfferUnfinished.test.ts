import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { useOfferUnfinished } from "../hooks/useOfferUnfinished";

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }));

const mockInvoke = vi.mocked(invoke);
const mockAsk = vi.mocked(ask);
const unfinished = { output_dir: "C:/SOP 2026-10-25 10-00-00", guide_title: "SOP 2026-10-25 10-00-00" };

const settle = () => act(async () => new Promise((r) => setTimeout(r, 0)));

describe("useOfferUnfinished", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockAsk.mockReset();
  });

  it("opens an unfinished recording for review when the user accepts", async () => {
    mockInvoke.mockResolvedValue(unfinished);
    mockAsk.mockResolvedValue(true);
    const review = vi.fn();
    renderHook(() => useOfferUnfinished(true, review));
    await settle();
    expect(mockInvoke).toHaveBeenCalledWith("unfinished_recording");
    expect(mockAsk.mock.calls[0][0]).toContain(unfinished.guide_title);
    expect(review).toHaveBeenCalledWith(unfinished.output_dir);
  });

  it("leaves it for later when the user declines", async () => {
    mockInvoke.mockResolvedValue(unfinished);
    mockAsk.mockResolvedValue(false);
    const review = vi.fn();
    renderHook(() => useOfferUnfinished(true, review));
    await settle();
    expect(review).not.toHaveBeenCalled();
  });

  it("asks nothing when there is no unfinished recording", async () => {
    mockInvoke.mockResolvedValue(null);
    renderHook(() => useOfferUnfinished(true, vi.fn()));
    await settle();
    expect(mockAsk).not.toHaveBeenCalled();
  });

  it("waits until the app is ready, then offers once per start", async () => {
    mockInvoke.mockResolvedValue(unfinished);
    mockAsk.mockResolvedValue(false);
    const { rerender } = renderHook(({ ready }) => useOfferUnfinished(ready, vi.fn()), {
      initialProps: { ready: false },
    });
    await settle();
    expect(mockInvoke).not.toHaveBeenCalled();

    rerender({ ready: true });
    await settle();
    rerender({ ready: false });
    rerender({ ready: true });
    await settle();
    expect(mockInvoke).toHaveBeenCalledTimes(1);
  });
});
