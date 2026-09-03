import { describe, it, expect } from "vitest";
import { expectedOutcome, isReportable } from "../lib/serverErrors";

/**
 * Design D6: not every `Err` is a defect. One case per exclusion, so a new
 * exclusion is a one-line change here as well as in the list itself.
 */
describe("which failures are reportable (design D6)", () => {
  it("a rate limit produces no report", () => {
    expect(
      expectedOutcome(
        '{"error":"rate_limit","message":"Generation limit reached (10/10)."}',
      ),
    ).toBe("rate_limit");
    expect(isReportable("Generation limit reached (10/10).")).toBe(false);
  });

  it("a PII block produces no report", () => {
    expect(isReportable("Personenbezogene Daten erkannt. Erzeugung abgebrochen.")).toBe(
      false,
    );
    expect(expectedOutcome('{"error":"pii_blocked"}')).toBe("pii_blocked");
  });

  it("a recording with no screenshots produces no report", () => {
    expect(expectedOutcome("No screenshots found")).toBe("no_screenshots");
  });

  it("an expired session produces no report", () => {
    expect(expectedOutcome("Sitzung abgelaufen. Bitte erneut anmelden.")).toBe(
      "session_expired",
    );
  });

  it("a stop with no active recording produces no report", () => {
    expect(expectedOutcome("Keine aktive Aufnahme.")).toBe("no_active_recording");
  });

  it("a cancelled dialog produces no report", () => {
    expect(expectedOutcome("Vorgang vom Benutzer abgebrochen")).toBe("cancelled");
    expect(expectedOutcome("Operation cancelled")).toBe("cancelled");
  });

  it("an unexplained error is reportable", () => {
    // The shape a real defect arrives in: a server 5xx the UI cannot explain.
    expect(isReportable("Upload failed: 500 Internal Server Error - <html>")).toBe(true);
    expect(isReportable("called `Option::unwrap()` on a `None` value")).toBe(true);
    expect(expectedOutcome("Upload failed: 500")).toBeNull();
  });

  it("an empty message is not reportable", () => {
    expect(isReportable("")).toBe(false);
  });
});
