import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAuth } from "../hooks/useAuth";

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
});
