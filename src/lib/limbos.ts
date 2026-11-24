/** What the recorder could not see into, as `unrecorded.json` stores it. */
export type UnrecordedWindow =
  | { kind: "app"; name: string }
  | { kind: "unknown_app" }
  | { kind: "security_prompt" };

/** One time an unrecorded window was in front, in recording seconds. */
export interface UnrecordedVisit {
  window: UnrecordedWindow;
  start_seconds: number;
  end_seconds: number;
}

/** A limbo: the unrecorded windows between step `after` and the next step. */
export interface Limbo {
  /** The step it follows; null before the first step. */
  after: number | null;
  windows: UnrecordedWindow[];
}

/** Shorter visits are Alt-Tab cycling, not work the user did there. */
const MIN_VISIT_SECONDS = 1;

const same = (a: UnrecordedWindow, b: UnrecordedWindow) =>
  a.kind === b.kind &&
  (a.kind !== "app" || (b.kind === "app" && a.name === b.name));

/**
 * The limbos of a recording: one per pair of neighbouring steps, each app
 * named once, in the order they were first in front. Computed from the steps
 * still there, so deleting a step merges the limbos on either side of it.
 */
export function limbos(
  steps: { step: number; timestamp_seconds: number }[],
  visits: UnrecordedVisit[],
): Limbo[] {
  const ordered = [...steps].sort(
    (a, b) => a.timestamp_seconds - b.timestamp_seconds,
  );
  const rows: Limbo[] = [];
  for (const v of [...visits].sort(
    (a, b) => a.start_seconds - b.start_seconds,
  )) {
    if (v.end_seconds - v.start_seconds < MIN_VISIT_SECONDS) continue;
    const before = ordered.filter(
      (s) => s.timestamp_seconds <= v.start_seconds,
    );
    const after = before.length > 0 ? before[before.length - 1].step : null;
    let row = rows.find((r) => r.after === after);
    if (!row) {
      row = { after, windows: [] };
      rows.push(row);
    }
    if (!row.windows.some((w) => same(w, v.window))) row.windows.push(v.window);
  }
  return rows;
}
