import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import { ErrorReportModal } from "../components/ErrorReportModal";
import type { ErrorReport } from "../lib/tauri";

function report(overrides: Partial<ErrorReport> = {}): ErrorReport {
  return {
    schema_version: 1,
    report_id: "aabbccdd-1111-2222-3333-444444444444",
    kind: "command_error",
    occurred_at: "2026-09-03T12:00:00+00:00",
    app_version: "0.15.0",
    os: "macos",
    os_version: "15.6",
    arch: "aarch64",
    locale: "de_DE",
    phase: "processing",
    message: "Upload failed: 500 - keine Antwort",
    location: null,
    log_tail: ["[INFO] Recording started: <Anleitungsverzeichnis>/<Aufnahme>"],
    settings: null,
    job_id: null,
    comment: null,
    consent: "pending",
    ...overrides,
  };
}

const noop = () => {};

beforeEach(() => {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "error_report_path") {
      return "/Users/anna/Library/Application Support/com.cogniclone.recorder/error-reports/aabbccdd-1111-2222-3333-444444444444.json";
    }
    return null;
  });
});

describe("ErrorReportModal", () => {
  it("signed in: lists what is and is not in the report, and sends nothing until asked", async () => {
    const onGrant = vi.fn();
    render(
      <ErrorReportModal
        report={report()}
        loggedIn
        onGrant={onGrant}
        onDecline={noop}
        onClose={noop}
      />,
    );

    expect(screen.getByText("Der Bericht enthält:")).toBeInTheDocument();
    expect(screen.getByText("Der Bericht enthält nie:")).toBeInTheDocument();
    expect(screen.getByText("Screenshots, Ton oder Transkripte")).toBeInTheDocument();
    // Nothing has been transmitted at this point.
    expect(onGrant).not.toHaveBeenCalled();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Details anzeigen" }));
    const details = screen.getByTestId("error-report-details");
    expect(details).toHaveTextContent("Upload failed: 500");
    expect(details).toHaveTextContent("<Anleitungsverzeichnis>/<Aufnahme>");

    await user.type(
      screen.getByLabelText("Was hast du gerade gemacht? (optional)"),
      "Auf Generieren geklickt",
    );
    await user.click(screen.getByLabelText("Fehlerberichte künftig automatisch senden"));
    await user.click(screen.getByRole("button", { name: "Bericht senden" }));

    expect(onGrant).toHaveBeenCalledWith("Auf Generieren geklickt", true);
  });

  it("signed out: offers to send after the next sign-in and to reveal the file", async () => {
    render(
      <ErrorReportModal
        report={report({ kind: "command_error", phase: "login" })}
        loggedIn={false}
        onGrant={noop}
        onDecline={noop}
        onClose={noop}
      />,
    );

    expect(
      screen.getByRole("button", { name: "Nach der Anmeldung senden" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Der Bericht wird gespeichert und nach der nächsten Anmeldung gesendet/),
    ).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Bericht-Datei anzeigen" }));
    await waitFor(() =>
      expect(revealItemInDir).toHaveBeenCalledWith(
        expect.stringContaining("aabbccdd-1111-2222-3333-444444444444.json"),
      ),
    );
  });

  it("sent: shows the report number with a copy button", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    // After `userEvent.setup()`, which installs a clipboard stub of its own,
    // and defined rather than assigned because happy-dom exposes
    // `navigator.clipboard` through a getter.
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });

    render(
      <ErrorReportModal
        report={report()}
        loggedIn
        sent={{ report_id: report().report_id, number: "aabbccdd" }}
        onGrant={noop}
        onDecline={noop}
        onClose={noop}
      />,
    );

    expect(screen.getByText("Berichtsnummer: aabbccdd")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Kopieren" }));
    expect(writeText).toHaveBeenCalledWith("aabbccdd");
  });

  it("shows the number after sending, when the report itself is already gone", async () => {
    // Consent deletes the file, so the confirmation has to stand on its own.
    // Passing the report here would be passing something that no longer exists.
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });

    render(
      <ErrorReportModal
        report={null}
        loggedIn
        sent={{ report_id: report().report_id, number: "aabbccdd" }}
        onGrant={noop}
        onDecline={noop}
        onClose={noop}
      />,
    );

    expect(screen.getByText("Berichtsnummer: aabbccdd")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Kopieren" }));
    expect(writeText).toHaveBeenCalledWith("aabbccdd");
  });

  it("renders nothing when there is neither a report nor a confirmation", () => {
    const { container } = render(
      <ErrorReportModal
        report={null}
        loggedIn
        onGrant={noop}
        onDecline={noop}
        onClose={noop}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("a crash found at launch says so", () => {
    render(
      <ErrorReportModal
        report={report({ kind: "panic", phase: "recording" })}
        loggedIn
        onGrant={noop}
        onDecline={noop}
        onClose={noop}
      />,
    );

    expect(
      screen.getByText("CogniClone wurde beim letzten Mal unerwartet beendet"),
    ).toBeInTheDocument();
  });

  it("a decline transmits nothing", async () => {
    const onDecline = vi.fn();
    const onGrant = vi.fn();
    render(
      <ErrorReportModal
        report={report()}
        loggedIn
        onGrant={onGrant}
        onDecline={onDecline}
        onClose={noop}
      />,
    );

    await userEvent.setup().click(screen.getByRole("button", { name: "Nicht senden" }));
    expect(onDecline).toHaveBeenCalled();
    expect(onGrant).not.toHaveBeenCalled();
  });
});
