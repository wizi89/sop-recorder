import { useState, type FormEvent } from "react";
import { useTranslation } from "../hooks/useTranslation";

/**
 * Sign-in.
 *
 * The primary path opens the user's real browser, where their password manager
 * works -- a Tauri window is a system webview with no extension host, so no
 * password manager can ever fill a form in here.
 *
 * The email/password form survives one release behind "sign in without a
 * browser": the browser handoff has failure modes the form does not (a browser
 * that cannot reach 127.0.0.1, an endpoint-security product that blocks a
 * desktop app from listening, no default browser), and the blast radius is the
 * whole product. Its removal is the first task of the follow-up change.
 */
interface LoginScreenProps {
  onLogin: (email: string, password: string) => Promise<void>;
  onLoginViaBrowser: () => Promise<void>;
  onCancelBrowserLogin: () => Promise<void>;
  awaitingBrowser: boolean;
  browserUnsupported: boolean;
  loading: boolean;
  error: string | null;
  version: string;
}

export function LoginScreen({
  onLogin,
  onLoginViaBrowser,
  onCancelBrowserLogin,
  awaitingBrowser,
  browserUnsupported,
  loading,
  error,
  version,
}: LoginScreenProps) {
  const { t } = useTranslation();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  // The fallback is revealed on request -- except when the server turns out
  // not to support browser sign-in, where it is the only way in.
  const [showFallback, setShowFallback] = useState(false);
  const fallbackVisible = showFallback || browserUnsupported;

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!email.trim() || !password) return;
    onLogin(email.trim(), password);
  };

  const openWebapp = async (path: string) => {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    const { getWebappUrl } = await import("../lib/tauri");
    const baseUrl = await getWebappUrl();
    await openUrl(`${baseUrl}${path}`);
  };

  const logo = (
    <div className="flex items-center gap-3 mb-2">
      <img src="/logo.svg" alt="CogniClone" style={{ width: 40, height: 40 }} />
      <div style={{ fontFamily: "'Orbitron', sans-serif", fontSize: "1.1rem", fontWeight: 600, letterSpacing: "0.06em" }}>
        <span style={{ color: "#A8B2B8" }}>COGNICLONE</span>
        <span style={{ color: "#2CB5C0", marginLeft: 6 }}>AI</span>
      </div>
    </div>
  );

  const links = (
    <div className="w-72 flex justify-between">
      <button
        type="button"
        className="bg-transparent border-none cursor-pointer text-on-surface-variant hover:text-on-surface"
        style={{ fontSize: "0.6875rem", transition: "color 0.15s" }}
        onClick={() => openWebapp("/login?forgot=1")}
      >
        {t("login.forgot_password")}
      </button>
      <button
        type="button"
        className="bg-transparent border-none cursor-pointer text-on-surface-variant hover:text-on-surface"
        style={{ fontSize: "0.6875rem", transition: "color 0.15s" }}
        onClick={() => openWebapp("/signup")}
      >
        {t("login.create_account")}
      </button>
    </div>
  );

  const versionLine = (
    <div className="px-4 pb-3 text-right">
      <span style={{ fontSize: "0.625rem", color: "#6B7780" }}>v{version}</span>
    </div>
  );

  // -- waiting for the browser ---------------------------------------------
  if (awaitingBrowser) {
    return (
      <div className="flex flex-col h-full bg-surface">
        <div className="flex-1 flex flex-col items-center justify-center gap-5 px-4">
          {logo}
          <p className="w-72 text-center text-sm">{t("login.browser_waiting")}</p>
          <p
            className="w-72 text-center text-on-surface-variant"
            style={{ fontSize: "0.6875rem" }}
          >
            {t("login.browser_waiting_hint")}
          </p>
          <button
            type="button"
            onClick={onCancelBrowserLogin}
            className="btn-secondary w-72 py-3 text-sm"
          >
            {t("login.browser_cancel")}
          </button>
        </div>
        {versionLine}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-surface">
      <div className="flex-1 flex flex-col items-center justify-center gap-5 px-4">
        {logo}

        {!fallbackVisible && (
          <>
            <button
              type="button"
              disabled={loading}
              onClick={onLoginViaBrowser}
              className="btn-primary w-72 py-3 text-sm"
            >
              {loading ? t("login.signing_in") : t("login.browser_sign_in")}
            </button>

            {error && (
              <p className="text-error w-72" style={{ fontSize: "0.75rem" }}>
                {error}
              </p>
            )}

            {links}

            <button
              type="button"
              className="bg-transparent border-none cursor-pointer text-on-surface-variant hover:text-on-surface"
              style={{ fontSize: "0.6875rem", transition: "color 0.15s" }}
              onClick={() => setShowFallback(true)}
            >
              {t("login.without_browser")}
            </button>
          </>
        )}

        {fallbackVisible && (
          <>
            {error && (
              <p className="text-error w-72" style={{ fontSize: "0.75rem" }}>
                {error}
              </p>
            )}

            <form onSubmit={handleSubmit} className="flex flex-col items-center gap-5">
              <div className="w-72">
                <label htmlFor="login-email" className="label-sm block mb-1.5">
                  {t("login.email")}
                </label>
                <input
                  id="login-email"
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="input-field w-full rounded-lg px-3.5 py-2.5 text-sm"
                  autoFocus
                />
              </div>

              <div className="w-72">
                <label htmlFor="login-password" className="label-sm block mb-1.5">
                  {t("login.password")}
                </label>
                <input
                  id="login-password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="input-field w-full rounded-lg px-3.5 py-2.5 text-sm"
                />
              </div>

              <button type="submit" disabled={loading} className="btn-primary w-72 py-3 text-sm">
                {loading ? t("login.signing_in") : t("login.sign_in")}
              </button>
            </form>

            {links}

            {/* Not offered when the server has no browser path: the click
                could not take effect, and a control that does nothing reads
                as the app being broken rather than the server being old. */}
            {!browserUnsupported && (
              <button
                type="button"
                className="bg-transparent border-none cursor-pointer text-on-surface-variant hover:text-on-surface"
                style={{ fontSize: "0.6875rem", transition: "color 0.15s" }}
                onClick={() => setShowFallback(false)}
              >
                {t("login.with_browser_instead")}
              </button>
            )}
          </>
        )}
      </div>
      {versionLine}
    </div>
  );
}
