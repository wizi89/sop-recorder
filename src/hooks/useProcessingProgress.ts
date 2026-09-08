import { useEffect, useState } from "react";
import { useElapsedTime } from "./useElapsedTime";

/**
 * How long a silence has to run before the user is told the app is still
 * waiting. Long enough that ordinary gaps between server status messages pass
 * unremarked, short enough to answer "is this thing still running?" before the
 * user goes looking for an answer elsewhere.
 */
export const STALL_NOTICE_AFTER_MS = 20_000;

/**
 * Progress signals for the generation wait.
 *
 * Generation takes minutes and the server's status messages are sparse, so the
 * screen could sit unchanged long enough that a working app and a hung one
 * looked identical -- the 2026-09-03 tester did not know whether anything was
 * running. Elapsed time answers that continuously; the stall notice answers it
 * explicitly once a silence gets long enough to worry about.
 *
 * The signal is the *displayed* message changing. A server that re-sends the
 * same text is indistinguishable here from one that sent nothing -- React sees
 * an unchanged prop either way -- and that is the right semantic regardless:
 * what the notice answers is "the screen has not changed in a while, is this
 * still running?", and an unchanged screen is exactly the condition.
 */
export function useProcessingProgress(
  processing: boolean,
  statusMessage: string,
): { elapsedSec: number; stalled: boolean } {
  const elapsedSec = useElapsedTime(processing);
  const [stalled, setStalled] = useState(false);

  useEffect(() => {
    if (!processing) {
      setStalled(false);
      return;
    }
    setStalled(false);
    const timer = setTimeout(() => setStalled(true), STALL_NOTICE_AFTER_MS);
    return () => clearTimeout(timer);
  }, [processing, statusMessage]);

  return { elapsedSec, stalled };
}
