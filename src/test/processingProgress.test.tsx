import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { renderHook } from "@testing-library/react";
import { StatusBar } from "../components/StatusBar";
import {
  useProcessingProgress,
  STALL_NOTICE_AFTER_MS,
} from "../hooks/useProcessingProgress";

/**
 * Generation takes minutes and the server's status messages are sparse, so the
 * screen could sit unchanged long enough that a working app and a hung one
 * looked identical. The 2026-09-03 tester did not know whether anything was
 * running at all.
 */
describe("useProcessingProgress", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("counts elapsed time while processing", () => {
    const { result } = renderHook(() => useProcessingProgress(true, "Analysiere..."));

    expect(result.current.elapsedSec).toBe(0);
    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(result.current.elapsedSec).toBeGreaterThanOrEqual(4);
  });

  it("says it is still waiting after a long silence", () => {
    const { result } = renderHook(() => useProcessingProgress(true, "Analysiere..."));

    expect(result.current.stalled).toBe(false);
    act(() => {
      vi.advanceTimersByTime(STALL_NOTICE_AFTER_MS + 100);
    });
    expect(result.current.stalled).toBe(true);
  });

  it("clears the notice when the next status message arrives", () => {
    const { result, rerender } = renderHook(
      ({ msg }) => useProcessingProgress(true, msg),
      { initialProps: { msg: "Analysiere..." } },
    );

    act(() => {
      vi.advanceTimersByTime(STALL_NOTICE_AFTER_MS + 100);
    });
    expect(result.current.stalled).toBe(true);

    rerender({ msg: "PDF wird erstellt..." });
    expect(result.current.stalled).toBe(false);
  });

  it("restarts the countdown on each new message rather than warning mid-run", () => {
    const { result, rerender } = renderHook(
      ({ msg }) => useProcessingProgress(true, msg),
      { initialProps: { msg: "Analysiere..." } },
    );

    act(() => {
      vi.advanceTimersByTime(STALL_NOTICE_AFTER_MS - 1_000);
    });
    expect(result.current.stalled).toBe(false);

    // A steady stream of progress messages never trips the notice, however
    // long the whole run takes.
    for (const msg of ["Schritt 1...", "Schritt 2...", "Schritt 3..."]) {
      rerender({ msg });
      act(() => {
        vi.advanceTimersByTime(STALL_NOTICE_AFTER_MS - 1_000);
      });
      expect(result.current.stalled).toBe(false);
    }
  });

  it("reports nothing when not processing", () => {
    const { result } = renderHook(() => useProcessingProgress(false, ""));

    act(() => {
      vi.advanceTimersByTime(STALL_NOTICE_AFTER_MS * 3);
    });
    expect(result.current.stalled).toBe(false);
  });
});

describe("StatusBar during processing", () => {
  it("shows elapsed time and the waiting notice", () => {
    render(
      <StatusBar message="Analysiere..." busy elapsedSec={95} stalled />,
    );

    expect(screen.getByText(/1:35/)).toBeInTheDocument();
    expect(screen.getByText(/Server arbeitet noch/i)).toBeInTheDocument();
  });

  it("shows neither when idle", () => {
    render(<StatusBar message="Bereit" busy={false} elapsedSec={95} />);

    expect(screen.queryByText(/1:35/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Server arbeitet noch/i)).not.toBeInTheDocument();
  });
});
