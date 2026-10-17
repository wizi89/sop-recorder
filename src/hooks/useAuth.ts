import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  login as tauriLogin,
  loginViaBrowser as tauriLoginViaBrowser,
  cancelBrowserLogin as tauriCancelBrowserLogin,
  logout as tauriLogout,
  refreshSession,
  type SessionState,
} from "../lib/tauri";
import { t } from "../i18n";
import { safeUnlisten } from "../lib/safeUnlisten";

interface AuthState {
  loggedIn: boolean;
  email: string | null;
  loading: boolean;
  error: string | null;
  /** True while the browser is open and the recorder is waiting for it. */
  awaitingBrowser: boolean;
  /** The one failure whose answer is the fallback form, not "try again". */
  browserUnsupported: boolean;
}

/**
 * Turn what the command rejected with into something to show the user.
 *
 * The command returns fixed tokens for the cases the UI has to tell apart;
 * anything else is already a German message from the network layer.
 */
function describeBrowserFailure(error: unknown): {
  message: string;
  unsupported: boolean;
} {
  const raw = String(error);
  if (raw.includes("browser-login-unsupported")) {
    return { message: t("login.browser_unsupported"), unsupported: true };
  }
  if (raw.includes("browser-login-denied")) {
    return { message: t("login.browser_denied"), unsupported: false };
  }
  if (raw.includes("browser-login-verification")) {
    return { message: t("login.browser_verification"), unsupported: false };
  }
  if (raw.includes("abgebrochen")) {
    return { message: t("login.browser_cancelled"), unsupported: false };
  }
  if (raw.includes("nicht abgeschlossen")) {
    return { message: t("login.browser_timeout"), unsupported: false };
  }
  return { message: raw, unsupported: false };
}

export function useAuth() {
  const [state, setState] = useState<AuthState>({
    loggedIn: false,
    email: null,
    loading: true,
    error: null,
    awaitingBrowser: false,
    browserUnsupported: false,
  });

  useEffect(() => {
    refreshSession()
      .then((session: SessionState) => {
        setState({
          loggedIn: session.logged_in,
          email: session.email,
          loading: false,
          error: null,
          awaitingBrowser: false,
          browserUnsupported: false,
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
        awaitingBrowser: false,
        browserUnsupported: false,
      });
    });
    return () => {
      void unlisten.then(safeUnlisten);
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
        awaitingBrowser: false,
        browserUnsupported: false,
      });
    } catch (e) {
      setState((s) => ({
        ...s,
        loading: false,
        error: String(e),
      }));
    }
  }, []);

  const loginViaBrowser = useCallback(async () => {
    setState((s) => ({
      ...s,
      loading: true,
      error: null,
      awaitingBrowser: true,
      browserUnsupported: false,
    }));
    try {
      const session = await tauriLoginViaBrowser();
      setState({
        loggedIn: session.logged_in,
        email: session.email,
        loading: false,
        error: null,
        awaitingBrowser: false,
        browserUnsupported: false,
      });
    } catch (e) {
      const { message, unsupported } = describeBrowserFailure(e);
      setState((s) => ({
        ...s,
        loading: false,
        awaitingBrowser: false,
        error: message,
        browserUnsupported: unsupported,
      }));
    }
  }, []);

  const cancelBrowserLogin = useCallback(async () => {
    try {
      await tauriCancelBrowserLogin();
    } catch {
      // The attempt may have ended on its own; the command is a no-op then.
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await tauriLogout();
    } catch {
      // ignore
    }
    setState({
      loggedIn: false,
      email: null,
      loading: false,
      error: null,
      awaitingBrowser: false,
      browserUnsupported: false,
    });
  }, []);

  return { ...state, login, loginViaBrowser, cancelBrowserLogin, logout };
}
