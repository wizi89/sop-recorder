import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { PermissionsScreen } from "../components/PermissionsScreen";

const defaults = {
  micPermission: "granted" as const,
  screenRecordingPermission: "denied" as const,
  accessibilityPermission: "denied" as const,
  onRequestPermissions: vi.fn(),
  onRestart: vi.fn(),
  onSkip: vi.fn(),
};

describe("PermissionsScreen", () => {
  it("names every permission and says what it is for", () => {
    render(<PermissionsScreen {...defaults} />);
    // The whole point of the screen: a reason attached to each request,
    // rather than three bare OS dialogs.
    expect(screen.getByText(/Mikrofon/)).toBeInTheDocument();
    expect(screen.getByText(/gesprochene Erklärung/)).toBeInTheDocument();
    expect(screen.getByText(/Bildschirmaufnahme/)).toBeInTheDocument();
    expect(screen.getByText(/Bildschirmfoto/)).toBeInTheDocument();
    expect(screen.getByText(/Bedienungshilfen/)).toBeInTheDocument();
    expect(screen.getByText(/Klicks und Tastendrücke/)).toBeInTheDocument();
  });

  it("shows each permission's own state, not one verdict for all three", () => {
    render(<PermissionsScreen {...defaults} />);
    expect(screen.getAllByText("Erteilt")).toHaveLength(1);
    expect(screen.getAllByText("Fehlt")).toHaveLength(2);
  });

  it("requests every permission from one gesture", async () => {
    const onRequestPermissions = vi.fn();
    render(
      <PermissionsScreen {...defaults} onRequestPermissions={onRequestPermissions} />,
    );
    await userEvent.click(screen.getByRole("button", { name: /Alle Berechtigungen/ }));
    expect(onRequestPermissions).toHaveBeenCalledTimes(1);
  });

  it("offers a restart only once everything is granted", async () => {
    const { rerender } = render(<PermissionsScreen {...defaults} />);
    expect(screen.queryByRole("button", { name: /neu starten/i })).toBeNull();

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

  it("does not fire a second batch of prompts while one is in flight", async () => {
    const onRequestPermissions = vi.fn();
    render(
      <PermissionsScreen
        {...defaults}
        requesting
        onRequestPermissions={onRequestPermissions}
      />,
    );
    const button = screen.getByRole("button", { name: /Alle Berechtigungen/ });
    expect(button).toBeDisabled();
    await userEvent.click(button);
    expect(onRequestPermissions).not.toHaveBeenCalled();
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
});
