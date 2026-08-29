import { describe, it, expect, vi, afterEach } from "vitest";
import { emit, listen } from "@tauri-apps/api/event";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LoginScreen } from "../components/LoginScreen";
import { RecordingBar } from "../components/RecordingBar";
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
    onSignOut: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenFolder: vi.fn(),
    onRetry: vi.fn(),
    onDismissPii: vi.fn(),
    onDismissRateLimit: vi.fn(),
    onConfirmGeneration: vi.fn(),
    onCancelFromReview: vi.fn(),
    version: APP_VERSION,
  };

  it("shows ready state with start button", () => {
    render(<RecorderScreen {...defaults} />);
    expect(screen.getByText("Bereit")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /aufnahme starten/i }),
    ).toBeInTheDocument();
  });


  it("shows done message and open folder button", () => {
    render(<RecorderScreen {...defaults} status="done" />);
    expect(screen.getByText(/fertig/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /ordner/i }),
    ).toBeInTheDocument();
  });

  it("shows error and retry-from-disk button when outputDir is preserved", () => {
    render(
      <RecorderScreen
        {...defaults}
        status="error"
        error="Upload failed"
        outputDir="C:\\Users\\test\\output"
      />,
    );
    expect(screen.getByText("Upload failed")).toBeInTheDocument();
    // New label is "Aus Aufnahme erneut versuchen" which matches /erneut/i
    expect(
      screen.getByRole("button", { name: /erneut versuchen/i }),
    ).toBeInTheDocument();
  });

  it("does not show retry-from-disk button when outputDir is null", () => {
    render(
      <RecorderScreen {...defaults} status="error" error="Upload failed" />,
    );
    expect(screen.getByText("Upload failed")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /erneut versuchen/i }),
    ).toBeNull();
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

  it("shows PII disabled chip when skipPiiCheck is true", () => {
    render(<RecorderScreen {...defaults} skipPiiCheck={true} />);
    expect(screen.getByText(/sicherheitsprüfung deaktiviert/i)).toBeInTheDocument();
  });

  it("hides PII disabled chip when skipPiiCheck is false", () => {
    render(<RecorderScreen {...defaults} skipPiiCheck={false} />);
    expect(screen.queryByText(/sicherheitsprüfung deaktiviert/i)).not.toBeInTheDocument();
  });

  it("hides PII disabled chip by default", () => {
    render(<RecorderScreen {...defaults} />);
    expect(screen.queryByText(/sicherheitsprüfung deaktiviert/i)).not.toBeInTheDocument();
  });

  it("PII chip opens settings when clicked", async () => {
    const onOpenSettings = vi.fn();
    render(<RecorderScreen {...defaults} skipPiiCheck={true} onOpenSettings={onOpenSettings} />);

    await userEvent.click(screen.getByText(/sicherheitsprüfung deaktiviert/i));
    expect(onOpenSettings).toHaveBeenCalled();
  });

  it("hides PII chip during recording mode", () => {
    render(<RecorderScreen {...defaults} status="recording" skipPiiCheck={true} />);
    expect(screen.queryByText(/sicherheitsprüfung deaktiviert/i)).not.toBeInTheDocument();
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

// The recording bar lives in its own window and holds no recorder state, so
// what it owes the rest of the app is exactly one thing: the right event.
describe("RecordingBar", () => {
  // The global afterEach calls clearAllMocks, which forgets calls but keeps
  // implementations. Two tests here drive `listen` to fire a specific event,
  // and without this that implementation would leak into every test after
  // them and fire it there too.
  afterEach(() => {
    vi.mocked(listen).mockImplementation(
      (async () => () => {}) as unknown as typeof listen,
    );
  });

  it("emits bar:stop when the stop button is clicked", async () => {
    render(<RecordingBar />);

    await userEvent.click(
      screen.getByRole("button", { name: /aufnahme stoppen/i }),
    );
    expect(emit).toHaveBeenCalledWith("bar:stop");
  });

  it("emits bar:undo when the undo button is enabled and clicked", async () => {
    // Undo is disabled at zero captures, which is the state a fresh bar is in;
    // one captured step is what turns it on.
    vi.mocked(listen).mockImplementation((async (event: string, handler: unknown) => {
      if (event === "recording:step_captured") {
        (handler as (e: { payload: number }) => void)({ payload: 1 });
      }
      return () => {};
    }) as unknown as typeof listen);

    render(<RecordingBar />);

    const undo = await screen.findByRole("button", { name: /letzten schritt/i });
    await userEvent.click(undo);
    expect(emit).toHaveBeenCalledWith("bar:undo");
  });
  it("replaces the counter with a warning when the input goes silent", async () => {
    // A denied microphone does not fail the stream on macOS, it yields zeroed
    // samples -- so this warning is the only thing standing between the user
    // and an SOP with no narration.
    vi.mocked(listen).mockImplementation((async (event: string, handler: unknown) => {
      if (event === "recording:audio_silent") {
        (handler as (e: { payload: null }) => void)({ payload: null });
      }
      return () => {};
    }) as unknown as typeof listen);

    render(<RecordingBar />);

    expect(await screen.findByText(/kein ton/i)).toBeInTheDocument();
  });
});
