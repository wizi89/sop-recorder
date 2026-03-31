import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  login as tauriLogin,
  logout as tauriLogout,
  refreshSession,
  type SessionState,
} from "../lib/tauri";

interface AuthState {
  loggedIn: boolean;
  email: string | null;
  loading: boolean;
  error: string | null;
}

export function useAuth() {
  const [state, setState] = useState<AuthState>({
    loggedIn: false,
    email: null,
    loading: true,
    error: null,
  });

  useEffect(() => {
    refreshSession()
      .then((session: SessionState) => {
        setState({
          loggedIn: session.logged_in,
          email: session.email,
          loading: false,
          error: null,
        });
      })
      .catch(() => {
        setState((s) => ({ ...s, loading: false }));
      });
  }, []);

  // Backend emits auth:session_expired when token refresh is permanently
  // exhausted.  Force the user back to the login screen with a clear message.
  useEffect(() => {
    const unlisten = listen("auth:session_expired", () => {
      setState({
        loggedIn: false,
        email: null,
        loading: false,
        error: "Sitzung abgelaufen. Bitte erneut anmelden.",
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    setState((s) => ({ ...s, loading: true, error: null }));
    try {
      const session = await tauriLogin(email, password);
      setState({
        loggedIn: session.logged_in,
        email: session.email,
        loading: false,
        error: null,
      });
    } catch (e) {
      setState((s) => ({
        ...s,
        loading: false,
        error: String(e),
      }));
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await tauriLogout();
    } catch {
      // ignore
    }
    setState({ loggedIn: false, email: null, loading: false, error: null });
  }, []);

  return { ...state, login, logout };
}
