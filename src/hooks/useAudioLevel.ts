import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Subscribe to the Rust-side `recording:audio_level` event and expose the
 * latest peak magnitude [0..1] from the microphone input.
 *
 * The Rust side emits at ~10 Hz. When no events arrive (not recording, or
 * the audio device is silent), the hook returns 0. A simple decay toward
 * zero when new events stop arriving is NOT applied here -- the Rust poller
 * naturally drives the atomic to whatever the callback wrote, and the
 * callback rewrites every audio buffer. So silence is visible immediately.
 */
export function useAudioLevel(): number {
  const [level, setLevel] = useState(0);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<number>("recording:audio_level", (event) => {
      setLevel(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  return level;
}
