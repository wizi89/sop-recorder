import { useState, type FormEvent } from "react";
import { useTranslation } from "../hooks/useTranslation";

interface LoginScreenProps {
  onLogin: (email: string, password: string) => Promise<void>;
  loading: boolean;
  error: string | null;
  version: string;
}

export function LoginScreen({
  onLogin,
  loading,
  error,
  version,
}: LoginScreenProps) {
  const { t } = useTranslation();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!email.trim() || !password) return;
    onLogin(email.trim(), password);
  };

  return (
    <div className="flex flex-col h-full bg-surface">
      {/* Form */}
      <form onSubmit={handleSubmit} className="flex-1 flex flex-col items-center justify-center gap-5 px-4">
        {/* CogniClone Logo */}
        <div className="flex items-center gap-3 mb-2">
          <img src="/logo.svg" alt="CogniClone" style={{ width: 40, height: 40 }} />
          <div style={{ fontFamily: "'Orbitron', sans-serif", fontSize: "1.1rem", fontWeight: 600, letterSpacing: "0.06em" }}>
            <span style={{ color: "#A8B2B8" }}>COGNICLONE</span>
            <span style={{ color: "#2CB5C0", marginLeft: 6 }}>AI</span>
          </div>
        </div>

        <div className="w-72">
          <label htmlFor="login-email" className="label-sm block mb-1.5">{t("login.email")}</label>
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
          <label htmlFor="login-password" className="label-sm block mb-1.5">{t("login.password")}</label>
          <input
            id="login-password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="input-field w-full rounded-lg px-3.5 py-2.5 text-sm"
          />
        </div>

        <div className="w-72 flex justify-between">
          <button
            type="button"
            className="bg-transparent border-none cursor-pointer text-on-surface-variant hover:text-on-surface"
            style={{ fontSize: "0.6875rem", transition: "color 0.15s" }}
            onClick={async () => {
              const { openUrl } = await import("@tauri-apps/plugin-opener");
              const { getWebappUrl } = await import("../lib/tauri");
              const baseUrl = await getWebappUrl();
              await openUrl(`${baseUrl}/login?forgot=1`);
            }}
          >
            {t("login.forgot_password")}
          </button>
          <button
            type="button"
            className="bg-transparent border-none cursor-pointer text-on-surface-variant hover:text-on-surface"
            style={{ fontSize: "0.6875rem", transition: "color 0.15s" }}
            onClick={async () => {
              const { openUrl } = await import("@tauri-apps/plugin-opener");
              const { getWebappUrl } = await import("../lib/tauri");
              const baseUrl = await getWebappUrl();
              await openUrl(`${baseUrl}/signup`);
            }}
          >
            {t("login.create_account")}
          </button>
        </div>

        {error && (
          <p className="text-error w-72" style={{ fontSize: "0.75rem" }}>{error}</p>
        )}

        <button
          type="submit"
          disabled={loading}
          className="btn-primary w-72 py-3 text-sm"
        >
          {loading ? t("login.signing_in") : t("login.sign_in")}
        </button>
      </form>

      {/* Version */}
      <div className="px-4 pb-3 text-right">
        <span style={{ fontSize: "0.625rem", color: "#6B7780" }}>
          v{version}
        </span>
      </div>
    </div>
  );
}
