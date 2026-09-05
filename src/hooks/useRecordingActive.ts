import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeUnlisten } from "../lib/safeUnlisten";

/**
 * Whether a recording is currently running, from the Rust side's own events.
 *
 * The recording bar lives in a window created once at startup and only hidden
 * between recordings, so it never remounts and has no mount to treat as the
 * start of a session. Without a boundary the timer counted from app launch,
 * the capture count showed the previous recording's total until the first new
 * screenshot, and the "Kein Ton" warning -- which latches on purpose -- stayed
 * on screen for the rest of the process once it had fired.
 */
export function useRecordingActive(): boolean {
  const [active, setActive] = useState(false);

  useEffect(() => {
    let unlisten: UnlistenFn[] = [];
    let cancelled = false;

    void Promise.all([
      listen("recording:started", () => setActive(true)),
      listen("recording:stopped", () => setActive(false)),
    ]).then((fns) => {
      if (cancelled) fns.forEach(safeUnlisten);
      else unlisten = fns;
    });

    return () => {
      cancelled = true;
      unlisten.forEach(safeUnlisten);
    };
  }, []);

  return active;
}
