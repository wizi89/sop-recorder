import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Track the number of screenshots captured during the active recording.
 *
 * Listens to two Rust-side events:
 *   - `recording:step_captured` fires after a screenshot is written; payload
 *     is the new step_num (1-indexed), which is the current total count.
 *   - `recording:step_deleted` fires when the user hits the undo control;
 *     payload is the new total count after deletion.
 *
 * The counter resets to 0 whenever `recording` transitions from false -> true
 * so a fresh recording always starts at zero.
 */
export function useCaptureCount(recording: boolean): number {
  const [count, setCount] = useState(0);

  // Reset to zero at the start of each recording.
  useEffect(() => {
    if (recording) setCount(0);
  }, [recording]);

  useEffect(() => {
    let unlistenCaptured: UnlistenFn | undefined;
    let unlistenDeleted: UnlistenFn | undefined;
    let cancelled = false;

    listen<number>("recording:step_captured", (event) => {
      // Payload is the step_num (1-indexed) for the newly-written screenshot.
      setCount(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenCaptured = fn;
      }
    });

    listen<number>("recording:step_deleted", (event) => {
      // Payload is the new authoritative count after deletion.
      setCount(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlistenDeleted = fn;
      }
    });

    return () => {
      cancelled = true;
      if (unlistenCaptured) unlistenCaptured();
      if (unlistenDeleted) unlistenDeleted();
    };
  }, []);

  return count;
}
