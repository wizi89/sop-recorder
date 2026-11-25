import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeUnlisten } from "../lib/safeUnlisten";

/**
 * True while an unrecorded window is in the foreground, as the Rust side
 * reports it. Cleared when a recording stops, so it never carries into the
 * next one. Not cleared on start: the Rust side reports the window already in
 * front before it sends `recording:started`.
 */
export function useUnrecordedWindow(recording: boolean): boolean {
  const [inFront, setInFront] = useState(false);

  useEffect(() => {
    if (!recording) setInFront(false);
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
