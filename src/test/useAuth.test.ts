import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "../hooks/useAuth";

const mockInvoke = vi.mocked(invoke);

describe("useAuth", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    // Default: get_session_state returns not logged in
    mockInvoke.mockResolvedValue({ logged_in: false, email: null });
  });

  it("attempts session restore on mount", async () => {
    mockInvoke.mockResolvedValue({ logged_in: false, email: null });
    const { result } = renderHook(() => useAuth());

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
});
