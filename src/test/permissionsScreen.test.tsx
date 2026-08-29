import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { PermissionsScreen } from "../components/PermissionsScreen";

const defaults = {
  micPermission: "granted" as const,
  screenRecordingPermission: "denied" as const,
  accessibilityPermission: "denied" as const,
  onRestart: vi.fn(),
  onSkip: vi.fn(),
};

describe("PermissionsScreen", () => {
  it("names every permission and says what it is for", () => {
    render(<PermissionsScreen {...defaults} />);
    // The whole point of the screen: a reason attached to each request,
    // rather than three bare OS dialogs.
    expect(screen.getByText("Mikrofon")).toBeInTheDocument();
    expect(screen.getByText(/gesprochene Erklärung/)).toBeInTheDocument();
    expect(screen.getByText("Bildschirmaufnahme")).toBeInTheDocument();
    expect(screen.getByText(/Bildschirmfoto/)).toBeInTheDocument();
    expect(screen.getByText("Bedienungshilfen")).toBeInTheDocument();
    expect(screen.getByText(/Klicks und Tastendrücke/)).toBeInTheDocument();
  });

  it("shows each permission's own state, not one verdict for all three", () => {
    render(<PermissionsScreen {...defaults} />);
    expect(screen.getAllByText("Erteilt")).toHaveLength(1);
    expect(screen.getAllByText("Fehlt")).toHaveLength(2);
  });


  it("offers a restart before everything is granted, not only after", async () => {
    // macOS reports a Screen Recording grant to a fresh process only, so with
    // that row still red the user has granted it and has no way to make the
    // app look again. Withholding the restart until everything is green is
    // what stranded them.
    const { rerender } = render(<PermissionsScreen {...defaults} />);
    expect(
      screen.getByRole("button", { name: /neu starten/i }),
    ).toBeInTheDocument();

    rerender(
      <PermissionsScreen
        {...defaults}
        screenRecordingPermission="granted"
        accessibilityPermission="granted"
      />,
    );
    // macOS applies these two only to a fresh process, so the restart is the
    // real last step of the flow, not a suggestion.
    expect(
      screen.getByRole("button", { name: /neu starten/i }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Alle Berechtigungen/ })).toBeNull();
  });

  it("can always be dismissed", async () => {
    // The mic state is a probe, not a real permission query. A wrong "missing"
    // must never lock anyone out of their own app.
    const onSkip = vi.fn();
    render(<PermissionsScreen {...defaults} onSkip={onSkip} />);
    await userEvent.click(screen.getByRole("button", { name: /Später/ }));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });

  it("routes a refused permission to its System Settings pane", async () => {
    // macOS shows a permission dialog only while the status is undetermined.
    // After a refusal the request call is a silent no-op, so without this the
    // screen offers a button that cannot work and never dismisses.
    render(
      <PermissionsScreen
        {...defaults}
        micPermission="denied"
        screenRecordingPermission="granted"
        accessibilityPermission="granted"
      />,
    );

    await userEvent.click(
      screen.getByRole("button", { name: /systemeinstellungen/i }),
    );
    expect(invoke).toHaveBeenCalledWith("open_privacy_settings", {
      pane: "microphone",
    });
  });

  it("does not offer a settings detour for a permission that can still be asked for", () => {
    // Undetermined is the case the grant-all button handles, so a second route
    // to the same grant would only be noise.
    render(
      <PermissionsScreen
        {...defaults}
        micPermission="undetermined"
        screenRecordingPermission="granted"
        accessibilityPermission="granted"
      />,
    );

    expect(
      screen.queryByRole("button", { name: /systemeinstellungen/i }),
    ).not.toBeInTheDocument();
  });
  it("drops the batch button when nothing is left that a prompt could grant", async () => {
    // The reported case: only the microphone missing, and already refused.
    // macOS will not raise a dialog for it again, so a primary button reading
    // "grant all permissions" did nothing at all when pressed.
    render(
      <PermissionsScreen
        {...defaults}
        micPermission="denied"
        screenRecordingPermission="granted"
        accessibilityPermission="granted"
      />,
    );

    expect(
      screen.queryByRole("button", { name: /alle berechtigungen/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /systemeinstellungen/i }),
    ).toBeInTheDocument();
  });

  it("grants the microphone from its own row while it can still be asked for", async () => {
    render(
      <PermissionsScreen
        {...defaults}
        micPermission="undetermined"
        screenRecordingPermission="granted"
        accessibilityPermission="granted"
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: /^erteilen$/i }));
    expect(invoke).toHaveBeenCalledWith("request_permission", {
      which: "microphone",
    });
  });

  it("sends screen recording to System Settings rather than offering a dialog", () => {
    // Screen Recording and Accessibility are switches in System Settings; their
    // prompts do no more than open that pane, so offering "Erteilen" would
    // promise a grant the dialog cannot make.
    render(
      <PermissionsScreen
        {...defaults}
        micPermission="granted"
        screenRecordingPermission="denied"
        accessibilityPermission="granted"
      />,
    );

    expect(
      screen.queryByRole("button", { name: /^erteilen$/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /systemeinstellungen/i }),
    ).toBeInTheDocument();
  });
  it("says which permission the OS will only notice after a restart", () => {
    // Screen Recording alone: CGPreflightScreenCaptureAccess caches its answer
    // for the life of the process, so a grant made in System Settings is
    // invisible until a fresh one asks. The other two report live.
    render(
      <PermissionsScreen
        {...defaults}
        micPermission="denied"
        screenRecordingPermission="denied"
        accessibilityPermission="denied"
      />,
    );

    expect(screen.getAllByText(/erst nach einem Neustart/i)).toHaveLength(1);
    // Same row, same reason it is a special case: macOS may refuse to list an
    // ad-hoc signed app at all, leaving the + button as the only way in.
    expect(screen.getAllByText(/mit \+ aus dem Programme-Ordner/i)).toHaveLength(1);
  });
});
