import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
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

describe("the dev triggers exercise both sides of the classifier", () => {
  // The `Command-Fehler` button once did nothing at all: it invoked the Rust
  // command, caught the rejection and logged it, without ever running the
  // classifier or creating a report. Nothing failed, because a button that
  // does nothing looks exactly like a button whose effect is elsewhere.
  // Reading the messages out of the Rust source keeps this honest even if
  // somebody edits them there.
  const source = readFileSync(
    resolve(__dirname, "../../src-tauri/src/commands/error_reports.rs"),
    "utf-8",
  );

  function messageFor(kind: string): string {
    const match = source.match(
      new RegExp(`"${kind}" => Err\\(\\s*"((?:[^"\\\\]|\\\\.)*)"`, "s"),
    );
    expect(match, `no Err message found for the ${kind} trigger`).not.toBeNull();
    return match![1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }

  it("the command_error trigger produces something reportable", () => {
    expect(isReportable(messageFor("command_error"))).toBe(true);
  });

  it("the expected_command_error trigger produces something suppressed", () => {
    const message = messageFor("expected_command_error");
    expect(isReportable(message)).toBe(false);
    expect(expectedOutcome(message)).toBe("no_active_recording");
  });
});
