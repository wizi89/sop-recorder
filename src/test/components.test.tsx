import { describe, it, expect, vi, afterEach } from "vitest";
import { emit, listen } from "@tauri-apps/api/event";
import { act, render, screen } from "@testing-library/react";
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
    onLoginViaBrowser: vi.fn(),
    onCancelBrowserLogin: vi.fn(),
    awaitingBrowser: false,
    browserUnsupported: false,
    loading: false,
    error: null,
    onOpenSettings: vi.fn(),
    version: APP_VERSION,
  };

  it("leads with the browser button and no password field", () => {
    render(<LoginScreen {...defaults} />);
    expect(
      screen.getByRole("button", { name: /mit browser anmelden/i }),
    ).toBeInTheDocument();
    // The recorder no longer asks for a password up front: a Tauri window is a
    // webview, so no password manager can fill one here.
    expect(screen.queryByLabelText(/passwort/i)).not.toBeInTheDocument();
  });

  it("invokes the browser sign-in", async () => {
    const onLoginViaBrowser = vi.fn().mockResolvedValue(undefined);
    render(<LoginScreen {...defaults} onLoginViaBrowser={onLoginViaBrowser} />);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /mit browser anmelden/i }));

    expect(onLoginViaBrowser).toHaveBeenCalled();
  });

  it("disables the browser button while loading", () => {
    render(<LoginScreen {...defaults} loading={true} />);
    expect(screen.getByRole("button", { name: /anmeldung/i })).toBeDisabled();
  });

  it("renders the waiting state while the browser is open", () => {
    render(<LoginScreen {...defaults} awaitingBrowser={true} />);
    expect(screen.getByText(/warte auf die anmeldung im browser/i)).toBeInTheDocument();
    expect(screen.getByText(/der browser wurde geöffnet/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /abbrechen/i })).toBeInTheDocument();
  });

  it("cancel in the waiting state invokes the command", async () => {
    const onCancelBrowserLogin = vi.fn().mockResolvedValue(undefined);
    render(
      <LoginScreen
        {...defaults}
        awaitingBrowser={true}
        onCancelBrowserLogin={onCancelBrowserLogin}
      />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /abbrechen/i }));

    expect(onCancelBrowserLogin).toHaveBeenCalled();
  });

  it("shows a timeout message", () => {
    render(
      <LoginScreen
        {...defaults}
        error="Die Anmeldung im Browser wurde nicht abgeschlossen. Du kannst es erneut versuchen."
      />,
    );
    expect(screen.getByText(/nicht abgeschlossen/i)).toBeInTheDocument();
  });

  it("shows error message", () => {
    render(<LoginScreen {...defaults} error="Invalid credentials" />);
    expect(screen.getByText("Invalid credentials")).toBeInTheDocument();
  });

  it("an unsupported server shows its own message and opens the fallback", () => {
    // The one login failure whose answer is the fallback rather than a retry:
    // the user cannot get in by trying again.
    render(
      <LoginScreen
        {...defaults}
        browserUnsupported={true}
        error="Dieser Server unterstützt die Anmeldung über den Browser noch nicht. Melde dich bitte ohne Browser an."
      />,
    );
    expect(screen.getByText(/unterstützt die anmeldung über den browser noch nicht/i))
      .toBeInTheDocument();
    // The form is already open: no extra click needed to get in.
    expect(screen.getByLabelText(/e-mail/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/passwort/i)).toBeInTheDocument();
  });

  it("the fallback form still submits email and password", async () => {
    const onLogin = vi.fn().mockResolvedValue(undefined);
    render(<LoginScreen {...defaults} onLogin={onLogin} />);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /ohne browser anmelden/i }));

    await user.type(screen.getByLabelText(/e-mail/i), "test@test.com");
    await user.type(screen.getByLabelText(/passwort/i), "password123");
    await user.click(screen.getByRole("button", { name: /^anmelden$/i }));

    expect(onLogin).toHaveBeenCalledWith("test@test.com", "password123");
  });

  it("the fallback can be closed again", async () => {
    render(<LoginScreen {...defaults} />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /ohne browser anmelden/i }));
    expect(screen.getByLabelText(/passwort/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /stattdessen mit browser/i }));
    expect(screen.queryByLabelText(/passwort/i)).not.toBeInTheDocument();
  });

  it("keeps the forgot-password and create-account links", () => {
    render(<LoginScreen {...defaults} />);
    expect(screen.getByRole("button", { name: /passwort vergessen/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /konto erstellen/i })).toBeInTheDocument();
  });

  it("shows version number", () => {
    render(<LoginScreen {...defaults} />);
    expect(screen.getByText(`v${APP_VERSION}`)).toBeInTheDocument();
  });

  it("shows the version in the waiting state too", () => {
    render(<LoginScreen {...defaults} awaitingBrowser={true} />);
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
  it("clears the silence warning when the next recording starts", async () => {
    // The bar's window is created once at startup and only hidden between
    // recordings, so nothing here ever remounts. Without a session boundary
    // the latch stayed lit for the rest of the process, reporting a fault that
    // had already been fixed.
    const handlers: Record<string, (e: { payload: unknown }) => void> = {};
    vi.mocked(listen).mockImplementation((async (event: string, handler: unknown) => {
      handlers[event] = handler as (e: { payload: unknown }) => void;
      return () => {};
    }) as unknown as typeof listen);

    render(<RecordingBar />);

    await act(async () => {
      handlers["recording:audio_silent"]?.({ payload: null });
    });
    expect(screen.getByText(/kein ton/i)).toBeInTheDocument();

    await act(async () => {
      handlers["recording:started"]?.({ payload: null });
    });
    expect(screen.queryByText(/kein ton/i)).not.toBeInTheDocument();
  });
});
