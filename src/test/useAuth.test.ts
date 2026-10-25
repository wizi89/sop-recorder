import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { describeBrowserFailure, RESTORE_RETRY_MS, useAuth } from "../hooks/useAuth";
import { t } from "../i18n";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

// Store event callbacks registered via listen() so tests can fire them
type EventCallback = (event: { payload: unknown }) => void;
const eventHandlers: Record<string, EventCallback[]> = {};

describe("useAuth", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    // Default: get_session_state returns not logged in
    mockInvoke.mockResolvedValue({ logged_in: false, email: null });

    // Clear captured event handlers
    for (const key of Object.keys(eventHandlers)) delete eventHandlers[key];

    // Capture listen callbacks so tests can simulate Tauri events
    mockListen.mockImplementation(((event: string, handler: EventCallback) => {
      if (!eventHandlers[event]) eventHandlers[event] = [];
      eventHandlers[event].push(handler);
      return Promise.resolve(() => {});
    }) as typeof listen);
  });

  it("attempts session restore on mount", async () => {
    mockInvoke.mockResolvedValue({ logged_in: false, email: null });
    renderHook(() => useAuth());

    // Wait for initial effect
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(mockInvoke).toHaveBeenCalledWith("refresh_session");
  });

  it("keeps the stored session through a server error and signs in once it answers", async () => {
    vi.useFakeTimers();
    try {
      mockInvoke
        .mockRejectedValueOnce("Refresh failed: server answered 503")
        .mockResolvedValueOnce({ logged_in: true, email: "a@b.de" });
      const { result } = renderHook(() => useAuth());
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });
      expect(result.current.loggedIn).toBe(false);
      expect(result.current.loading).toBe(false);
      expect(result.current.error).toBe(t("login.restore_unavailable"));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(RESTORE_RETRY_MS);
      });
      expect(mockInvoke).toHaveBeenCalledTimes(2);
      expect(result.current.loggedIn).toBe(true);
      expect(result.current.error).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows the sign-in screen without retrying on any other restore failure", async () => {
    vi.useFakeTimers();
    try {
      mockInvoke.mockRejectedValueOnce("Platform secure storage failure");
      const { result } = renderHook(() => useAuth());
      await act(async () => {
        await vi.advanceTimersByTimeAsync(RESTORE_RETRY_MS * 3);
      });
      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(result.current.loading).toBe(false);
      expect(result.current.error).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("sets loggedIn true on successful login", async () => {
    mockInvoke
      .mockResolvedValueOnce({ logged_in: false, email: null }) // refresh on mount
      .mockResolvedValueOnce({ logged_in: true, email: "test@test.com" }); // login

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    await act(async () => {
      await result.current.login("test@test.com", "pass123");
    });

    expect(result.current.loggedIn).toBe(true);
    expect(result.current.email).toBe("test@test.com");
  });

  it("sets error on failed login", async () => {
    mockInvoke
      .mockResolvedValueOnce({ logged_in: false, email: null }) // refresh on mount
      .mockRejectedValueOnce(new Error("Invalid credentials")); // login

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    await act(async () => {
      await result.current.login("bad@test.com", "wrong");
    });

    expect(result.current.loggedIn).toBe(false);
    expect(result.current.error).toContain("Invalid credentials");
  });

  it("clears state on logout", async () => {
    mockInvoke
      .mockResolvedValueOnce({ logged_in: true, email: "test@test.com" }) // refresh
      .mockResolvedValueOnce(undefined); // logout

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    await act(async () => {
      await result.current.logout();
    });

    expect(result.current.loggedIn).toBe(false);
    expect(result.current.email).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("logout");
  });

  it("listens for auth:session_expired event", async () => {
    renderHook(() => useAuth());

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(eventHandlers["auth:session_expired"]).toBeDefined();
    expect(eventHandlers["auth:session_expired"].length).toBe(1);
  });

  it("forces logout with error on auth:session_expired event", async () => {
    // Start logged in
    mockInvoke.mockResolvedValueOnce({ logged_in: true, email: "test@test.com" });

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(result.current.loggedIn).toBe(true);
    expect(result.current.email).toBe("test@test.com");

    // Simulate backend emitting auth:session_expired
    await act(async () => {
      for (const handler of eventHandlers["auth:session_expired"] ?? []) {
        handler({ payload: undefined });
      }
    });

    expect(result.current.loggedIn).toBe(false);
    expect(result.current.email).toBeNull();
    expect(result.current.error).toContain("Sitzung abgelaufen");
    expect(result.current.loading).toBe(false);
  });

  it("shows login screen with session-expired error after event", async () => {
    // User was logged in and actively using the app
    mockInvoke.mockResolvedValueOnce({ logged_in: true, email: "user@company.com" });

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Fire session_expired (simulates what happens when generate.rs
    // exhausts all token refresh attempts)
    await act(async () => {
      for (const handler of eventHandlers["auth:session_expired"] ?? []) {
        handler({ payload: undefined });
      }
    });

    // User can now re-login and the error tells them why
    expect(result.current.loggedIn).toBe(false);
    expect(result.current.error).toBe("Sitzung abgelaufen. Bitte erneut anmelden.");

    // After re-login the error should clear
    mockInvoke.mockResolvedValueOnce({ logged_in: true, email: "user@company.com" });

    await act(async () => {
      await result.current.login("user@company.com", "password");
    });

    expect(result.current.loggedIn).toBe(true);
    expect(result.current.error).toBeNull();
  });

  // -- Browser sign-in -------------------------------------------------------

  it("login_via_browser signs the user in", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "refresh_session") {
        return Promise.resolve({ logged_in: false, email: null });
      }
      if (cmd === "login_via_browser") {
        return Promise.resolve({ logged_in: true, email: "user@test.com" });
      }
      return Promise.resolve(undefined);
    });

    const { result } = renderHook(() => useAuth());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    await act(async () => {
      await result.current.loginViaBrowser();
    });

    expect(mockInvoke).toHaveBeenCalledWith("login_via_browser");
    expect(result.current.loggedIn).toBe(true);
    expect(result.current.email).toBe("user@test.com");
    expect(result.current.awaitingBrowser).toBe(false);
  });

  it("marks the attempt as awaiting the browser while it runs", async () => {
    let release: (value: unknown) => void = () => {};
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "refresh_session") {
        return Promise.resolve({ logged_in: false, email: null });
      }
      if (cmd === "login_via_browser") {
        return new Promise((resolve) => {
          release = resolve;
        });
      }
      return Promise.resolve(undefined);
    });

    const { result } = renderHook(() => useAuth());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    let pending: Promise<void> = Promise.resolve();
    await act(async () => {
      pending = result.current.loginViaBrowser();
      await new Promise((r) => setTimeout(r, 10));
    });
    expect(result.current.awaitingBrowser).toBe(true);

    await act(async () => {
      release({ logged_in: true, email: "user@test.com" });
      await pending;
    });
    expect(result.current.awaitingBrowser).toBe(false);
  });

  it("reports a cancelled attempt without an unsupported-server flag", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "refresh_session") {
        return Promise.resolve({ logged_in: false, email: null });
      }
      if (cmd === "login_via_browser") {
        return Promise.reject("Die Anmeldung wurde abgebrochen.");
      }
      return Promise.resolve(undefined);
    });

    const { result } = renderHook(() => useAuth());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    await act(async () => {
      await result.current.loginViaBrowser();
    });

    expect(result.current.loggedIn).toBe(false);
    expect(result.current.error).toContain("abgebrochen");
    expect(result.current.browserUnsupported).toBe(false);
    expect(result.current.awaitingBrowser).toBe(false);
  });

  it("flags a server without the endpoint so the UI can offer the fallback", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "refresh_session") {
        return Promise.resolve({ logged_in: false, email: null });
      }
      if (cmd === "login_via_browser") {
        return Promise.reject("browser-login-unsupported");
      }
      return Promise.resolve(undefined);
    });

    const { result } = renderHook(() => useAuth());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    await act(async () => {
      await result.current.loginViaBrowser();
    });

    expect(result.current.browserUnsupported).toBe(true);
    // Not the raw token: the user sees a sentence.
    expect(result.current.error).toContain("Browser");
    expect(result.current.error).not.toContain("browser-login-unsupported");
  });

  it("translates a decline and a verification failure distinctly", async () => {
    for (const [rejection, fragment] of [
      ["browser-login-denied", "abgelehnt"],
      ["browser-login-verification", "Sicherheitsgründen"],
    ]) {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "refresh_session") {
          return Promise.resolve({ logged_in: false, email: null });
        }
        if (cmd === "login_via_browser") return Promise.reject(rejection);
        return Promise.resolve(undefined);
      });

      const { result } = renderHook(() => useAuth());
      await act(async () => {
        await new Promise((r) => setTimeout(r, 50));
      });
      await act(async () => {
        await result.current.loginViaBrowser();
      });

      expect(result.current.error).toContain(fragment);
      expect(result.current.browserUnsupported).toBe(false);
    }
  });

  it("marks a browser-path failure for a report, and clears it on the next attempt", async () => {
    let rejection: string | null = "Die Anmeldung im Browser wurde nicht abgeschlossen.";
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "refresh_session") {
        return Promise.resolve({ logged_in: false, email: null });
      }
      if (cmd === "login_via_browser") {
        return rejection
          ? Promise.reject(rejection)
          : new Promise(() => {}); // the second attempt is still waiting
      }
      return Promise.resolve(undefined);
    });

    const { result } = renderHook(() => useAuth());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    await act(async () => {
      await result.current.loginViaBrowser();
    });
    // The raw command error, not the translated sentence: that is what a
    // report needs to be grouped by.
    expect(result.current.browserFailureToReport).toBe(
      "Die Anmeldung im Browser wurde nicht abgeschlossen.",
    );

    rejection = null;
    act(() => {
      void result.current.loginViaBrowser();
    });
    expect(result.current.browserFailureToReport).toBeNull();
  });

  it("cancel invokes the command and survives it failing", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "refresh_session") {
        return Promise.resolve({ logged_in: false, email: null });
      }
      if (cmd === "cancel_browser_login") {
        // The attempt may already have ended; the command is a no-op then.
        return Promise.reject("no attempt");
      }
      return Promise.resolve(undefined);
    });

    const { result } = renderHook(() => useAuth());
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    await act(async () => {
      await result.current.cancelBrowserLogin();
    });
    expect(mockInvoke).toHaveBeenCalledWith("cancel_browser_login");
  });
});

// Which failures say something about the browser path. The in-app form stays,
// so these reports are the only signal that browser sign-in fails for anyone.
describe("describeBrowserFailure", () => {
  it.each([
    ["browser-login-unsupported", false],
    ["browser-login-denied", false],
    ["Die Anmeldung wurde abgebrochen.", false],
    ["browser-login-verification", true],
    ["Die Anmeldung im Browser wurde nicht abgeschlossen.", true],
    ["Could not open the local callback port: Address already in use", true],
    ["Der Browser konnte nicht geöffnet werden: no handler", true],
    ["Connection failed: dns error", true],
  ])("%s -> reportable %s", (rejection, reportable) => {
    expect(describeBrowserFailure(rejection).reportable).toBe(reportable);
  });
});
