import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeUnlisten } from "../lib/safeUnlisten";

/**
 * True while an unrecorded window is in the foreground, as the Rust side
 * reports it. Cleared at every session boundary, so it never carries into the
 * next recording.
 */
export function useUnrecordedWindow(recording: boolean): boolean {
  const [inFront, setInFront] = useState(false);

  useEffect(() => {
    setInFront(false);
  }, [recording]);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    void listen<boolean>("recording:unrecorded_window", (e) =>
      setInFront(e.payload === true),
    ).then((fn) => {
      if (cancelled) safeUnlisten(fn);
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      safeUnlisten(unlisten);
    };
  }, []);

  return inFront;
}
