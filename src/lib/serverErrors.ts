/**
 * Parsers for structured error payloads surfaced by the FastAPI server.
 *
 * The server responds with JSON bodies of the shape
 *   {"error": "rate_limit", "message": "Generation limit reached (10/10). ..."}
 * The Rust side wraps those into thrown strings / SSE `error` events containing
 * the full JSON fragment. These helpers extract structured info so the UI can
 * branch on error code instead of dumping raw JSON in front of the user.
 */

export interface RateLimitInfo {
  count: number | null;
  limit: number | null;
}

/**
 * Returns parsed rate-limit info if the error string corresponds to the
 * server's `rate_limit` response, otherwise null.
 *
 * Handles both the machine-readable JSON fragment and the German/English
 * human-readable message. Numbers are optional -- if the server message
 * format changes, we still surface the modal, just without the counters.
 */
export function parseRateLimit(msg: string): RateLimitInfo | null {
  if (!msg) return null;
  const looksLikeRateLimit =
    msg.includes('"error":"rate_limit"') ||
    msg.includes('"error": "rate_limit"') ||
    msg.includes("Generation limit reached") ||
    msg.includes("rate_limit");
  if (!looksLikeRateLimit) return null;

  // Try to extract "(count/limit)" from the human message
  const paren = msg.match(/\((\d+)\s*\/\s*(\d+)\)/);
  if (paren) {
    return { count: parseInt(paren[1], 10), limit: parseInt(paren[2], 10) };
  }
  return { count: null, limit: null };
}

/**
 * Whether a failure is worth offering a report for (design D6).
 *
 * Not every `Err` is a defect. The recorder presents several outcomes as
 * normal -- a quota that ran out, a PII block, a recording with no clicks in
 * it -- and offering to report those would train the user to ignore the offer
 * for the failures that are defects.
 *
 * The list is here, in one function with a test per entry, so that adding an
 * exclusion is a one-line change with a one-line test, and so that reading the
 * list tells you what the product considers normal.
 */
const EXPECTED_OUTCOMES: ReadonlyArray<{ id: string; matches: (msg: string) => boolean }> = [
  // The quota ran out. There is a modal for this, and an upgrade path.
  { id: "rate_limit", matches: (m) => parseRateLimit(m) !== null },
  // Personal data stopped the generation. That is the check working.
  {
    id: "pii_blocked",
    matches: (m) =>
      m.includes("pii_blocked") ||
      m.includes("Personenbezogene Daten"),
  },
  // The user recorded nothing. commands/generate.rs:106.
  { id: "no_screenshots", matches: (m) => m.includes("No screenshots found") },
  // The token ran out; the user signs in again. commands/generate.rs:220.
  { id: "session_expired", matches: (m) => m.includes("Sitzung abgelaufen") },
  // Stop pressed with nothing running. commands/recording.rs:257.
  { id: "no_active_recording", matches: (m) => m.includes("Keine aktive Aufnahme") },
  // The user closed a dialog. Their decision, not a failure.
  {
    id: "cancelled",
    matches: (m) => /\b(abgebrochen|cancelled|canceled)\b/i.test(m),
  },
];

/** The id of the expected outcome this message is, or null if it is a defect. */
export function expectedOutcome(msg: string): string | null {
  if (!msg) return "empty";
  return EXPECTED_OUTCOMES.find((o) => o.matches(msg))?.id ?? null;
}

/** Whether the recorder should offer to report this failure. */
export function isReportable(msg: string): boolean {
  return expectedOutcome(msg) === null;
}
