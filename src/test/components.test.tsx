import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LoginScreen } from "../components/LoginScreen";
import { RecorderScreen } from "../components/RecorderScreen";
import { StatusBar } from "../components/StatusBar";
import tauriConf from "../../src-tauri/tauri.conf.json";

const APP_VERSION = tauriConf.version;

describe("LoginScreen", () => {
  const defaults = {
    onLogin: vi.fn(),
    loading: false,
    error: null,
    onOpenSettings: vi.fn(),
    version: APP_VERSION,
  };

  it("renders email and password fields", () => {
    render(<LoginScreen {...defaults} />);
    expect(screen.getByLabelText(/e-mail/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/passwort/i)).toBeInTheDocument();
  });

  it("renders sign-in button", () => {
    render(<LoginScreen {...defaults} />);
    expect(screen.getByRole("button", { name: /anmelden/i })).toBeInTheDocument();
  });

  it("disables button when loading", () => {
    render(<LoginScreen {...defaults} loading={true} />);
    expect(screen.getByRole("button", { name: /anmeldung/i })).toBeDisabled();
  });

  it("shows error message", () => {
    render(<LoginScreen {...defaults} error="Invalid credentials" />);
    expect(screen.getByText("Invalid credentials")).toBeInTheDocument();
  });

  it("calls onLogin with email and password", async () => {
    const onLogin = vi.fn().mockResolvedValue(undefined);
    render(<LoginScreen {...defaults} onLogin={onLogin} />);

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/e-mail/i), "test@test.com");
    await user.type(screen.getByLabelText(/passwort/i), "password123");
    await user.click(screen.getByRole("button", { name: /anmelden/i }));

    expect(onLogin).toHaveBeenCalledWith("test@test.com", "password123");
  });

  it("shows version number", () => {
    render(<LoginScreen {...defaults} />);
    expect(screen.getByText(`v${APP_VERSION}`)).toBeInTheDocument();
  });
});

describe("RecorderScreen", () => {
  const defaults = {
    email: "user@test.com",
    status: "idle" as const,
    statusMessage: "",
    error: null,
    outputDir: null,
    onStart: vi.fn(),
    onStop: vi.fn(),
    onCancel: vi.fn(),
    onSignOut: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenFolder: vi.fn(),
    onRetry: vi.fn(),
    onDismissPii: vi.fn(),
    version: APP_VERSION,
  };

  it("shows ready state with start button", () => {
    render(<RecorderScreen {...defaults} />);
    expect(screen.getByText("Bereit")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /aufnahme starten/i }),
    ).toBeInTheDocument();
  });

  it("shows stop button in recording state", () => {
    render(<RecorderScreen {...defaults} status="recording" />);
    expect(
      screen.getByRole("button", { name: /aufnahme stoppen/i }),
    ).toBeInTheDocument();
  });

  it("shows done message and open folder button", () => {
    render(<RecorderScreen {...defaults} status="done" />);
    expect(screen.getByText(/fertig/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /ordner/i }),
    ).toBeInTheDocument();
  });

  it("shows error and retry button", () => {
    render(
      <RecorderScreen {...defaults} status="error" error="Upload failed" />,
    );
    expect(screen.getByText("Upload failed")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /erneut/i }),
    ).toBeInTheDocument();
  });

  it("shows user email", () => {
    render(<RecorderScreen {...defaults} />);
    expect(screen.getByText("user@test.com")).toBeInTheDocument();
  });

  it("calls onStart when start button clicked", async () => {
    const onStart = vi.fn();
    render(<RecorderScreen {...defaults} onStart={onStart} />);

    await userEvent.click(
      screen.getByRole("button", { name: /aufnahme starten/i }),
    );
    expect(onStart).toHaveBeenCalled();
  });

  it("calls onStop when stop button clicked", async () => {
    const onStop = vi.fn();
    render(<RecorderScreen {...defaults} status="recording" onStop={onStop} />);

    await userEvent.click(
      screen.getByRole("button", { name: /aufnahme stoppen/i }),
    );
    expect(onStop).toHaveBeenCalled();
  });
});

describe("StatusBar", () => {
  it("shows message text", () => {
    render(<StatusBar message="Processing..." busy={false} />);
    expect(screen.getByText("Processing...")).toBeInTheDocument();
  });

  it("shows progress bar when busy", () => {
    const { container } = render(<StatusBar message="Uploading..." busy={true} />);
    const progressBar = container.querySelector(".bg-primary");
    expect(progressBar).toBeInTheDocument();
  });

  it("hides progress bar when not busy", () => {
    const { container } = render(<StatusBar message="Done" busy={false} />);
    const progressBar = container.querySelector(".bg-primary");
    expect(progressBar).not.toBeInTheDocument();
  });

  it("applies error styling", () => {
    render(<StatusBar message="Error occurred" busy={false} isError={true} />);
    const el = screen.getByText("Error occurred");
    expect(el.className).toContain("text-error");
  });
});
