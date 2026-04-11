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
