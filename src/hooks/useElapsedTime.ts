import { useEffect, useState } from "react";

/**
 * Tracks elapsed wall-clock time since `running` became true.
 *
 * Returns the elapsed duration in whole seconds. On a false -> true
 * transition the counter resets to 0 and starts ticking. On a true -> false
 * transition the interval is torn down but the last elapsed value is FROZEN,
 * so downstream UIs (e.g. the post-recording review screen) can still display
 * how long the captured recording lasted.
 */
export function useElapsedTime(running: boolean): number {
  const [elapsedSec, setElapsedSec] = useState(0);

  useEffect(() => {
    if (!running) {
      // Freeze the last value -- do not reset. The next true transition
      // clears and restarts the counter.
      return;
    }
    const started = Date.now();
    setElapsedSec(0);
    const interval = setInterval(() => {
      setElapsedSec(Math.floor((Date.now() - started) / 1000));
    }, 250);
    return () => clearInterval(interval);
  }, [running]);

  return elapsedSec;
}

/** Format a seconds count as `M:SS`. */
export function formatElapsed(totalSec: number): string {
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
