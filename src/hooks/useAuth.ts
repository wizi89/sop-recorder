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
import { isReportable } from "../lib/serverErrors";

interface AuthState {
  loggedIn: boolean;
  email: string | null;
  loading: boolean;
  error: string | null;
  /** True while the browser is open and the recorder is waiting for it. */
  awaitingBrowser: boolean;
  /** The one failure whose answer is the fallback form, not "try again". */
  browserUnsupported: boolean;
  /**
   * The last browser sign-in failure worth an error report, as the command
   * reported it; null otherwise. The in-app form stays as the way in, so
   * these reports are the only sign that the browser path fails for someone.
   */
  browserFailureToReport: string | null;
}

/**
 * Turn what the command rejected with into something to show the user.
 *
 * The command returns fixed tokens for the cases the UI has to tell apart;
 * anything else is already a German message from the network layer.
 */
export function describeBrowserFailure(error: unknown): {
  message: string;
  unsupported: boolean;
  /** Whether the failure says something about the browser path (see below). */
  reportable: boolean;
} {
  const raw = String(error);
  // Not reported: a server without the endpoint is a deployment question the
  // release's server-contract check answers, and a denial or a cancel is the
  // user's own decision.
  if (raw.includes("browser-login-unsupported")) {
    return { message: t("login.browser_unsupported"), unsupported: true, reportable: false };
  }
  if (raw.includes("browser-login-denied")) {
    return { message: t("login.browser_denied"), unsupported: false, reportable: false };
  }
  if (raw.includes("browser-login-verification")) {
    return { message: t("login.browser_verification"), unsupported: false, reportable: true };
  }
  if (raw.includes("abgebrochen")) {
    return { message: t("login.browser_cancelled"), unsupported: false, reportable: false };
  }
  // Reported although a user who walked away produces it too: a browser that
  // cannot reach 127.0.0.1 -- the endpoint-security and local-network-access
  // risks in the browser-login design -- looks exactly like this.
  if (raw.includes("nicht abgeschlossen")) {
    return { message: t("login.browser_timeout"), unsupported: false, reportable: true };
  }
  // The port could not be bound, the browser would not open, the exchange
  // failed: reported unless it is an outcome the product treats as normal.
  return { message: raw, unsupported: false, reportable: isReportable(raw) };
}

/** How often a stored session is tried again while the server is unreachable. */
export const RESTORE_RETRY_MS = 10_000;

export function useAuth() {
  const [state, setState] = useState<AuthState>({
    loggedIn: false,
    email: null,
    loading: true,
    error: null,
    awaitingBrowser: false,
    browserUnsupported: false,
    browserFailureToReport: null,
  });

  useEffect(() => {
    let cancelled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const restore = () =>
      refreshSession()
        .then((session: SessionState) => {
          if (cancelled) return;
          // A retry must not undo a sign-in the user finished meanwhile.
          setState((s) =>
            s.loggedIn
              ? s
              : {
                  loggedIn: session.logged_in,
                  email: session.email,
                  loading: false,
                  error: null,
                  awaitingBrowser: false,
                  browserUnsupported: false,
                  browserFailureToReport: null,
                },
          );
        })
        .catch((e) => {
          if (cancelled) return;
          // The server could not be reached or answered with an error of its
          // own. The stored session is kept for exactly this case, so say so
          // and try again rather than asking for a new sign-in.
          if (String(e).startsWith("Refresh failed")) {
            setState((s) =>
              s.loggedIn ? s : { ...s, loading: false, error: t("login.restore_unavailable") },
            );
            retry = setTimeout(restore, RESTORE_RETRY_MS);
          } else {
            setState((s) => ({ ...s, loading: false }));
          }
        });
    restore();
    return () => {
      cancelled = true;
      clearTimeout(retry);
    };
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
        browserFailureToReport: null,
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
        browserFailureToReport: null,
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
      browserFailureToReport: null,
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
        browserFailureToReport: null,
      });
    } catch (e) {
      const { message, unsupported, reportable } = describeBrowserFailure(e);
      setState((s) => ({
        ...s,
        loading: false,
        awaitingBrowser: false,
        error: message,
        browserUnsupported: unsupported,
        browserFailureToReport: reportable ? String(e) : null,
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
      browserFailureToReport: null,
    });
  }, []);

  return { ...state, login, loginViaBrowser, cancelBrowserLogin, logout };
}
