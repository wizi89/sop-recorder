import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * True once the Rust side has reported that the input has been exactly silent
 * for several seconds.
 *
 * That is the signature of a denied microphone rather than a quiet room: macOS
 * does not fail a denied capture, it vends zeroed samples, so without this the
 * recording runs to completion and produces an SOP with no narration at all.
 * A live input always carries some noise floor.
 *
 * Latching is deliberate. The warning is about a recording that is already
 * compromised, and clearing it the moment a stray sample arrives would let the
 * indicator blink rather than say something.
 */
export function useAudioSilent(): boolean {
  const [silent, setSilent] = useState(false);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    void listen("recording:audio_silent", () => setSilent(true)).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return silent;
}
