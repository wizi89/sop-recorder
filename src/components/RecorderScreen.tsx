import { ask } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "../hooks/useTranslation";
import { StatusBar } from "./StatusBar";
import { PiiBlockedModal } from "./PiiBlockedModal";
import type { RecorderStatus } from "../hooks/useRecorder";

interface RecorderScreenProps {
  email: string | null;
  status: RecorderStatus;
  statusMessage: string;
  error: string | null;
  piiFindings?: unknown | null;
  outputDir: string | null;
  skipPiiCheck?: boolean;
  onStart: () => void;
  onStop: () => void;
  onCancel: () => void;
  onSignOut: () => void;
  onOpenSettings: () => void;
  onOpenFolder: () => void;
  onRetry: () => void;
  onDismissPii: () => void;
  version: string;
}

export function RecorderScreen({
  email,
  status,
  statusMessage,
  error,
  piiFindings: _piiFindings,
  skipPiiCheck,
  onStart,
  onStop,
  onCancel,
  onSignOut,
  onOpenSettings,
  onOpenFolder,
  onRetry,
  onDismissPii,
  version,
}: RecorderScreenProps) {
  const { t } = useTranslation();

  // Compact recording mode
  if (status === "recording") {
    const handleCancel = async () => {
      const appWindow = getCurrentWindow();
      await appWindow.setAlwaysOnTop(false);
      const confirmed = await ask(t("status.cancel_message"), {
        title: t("status.cancel_title"),
        kind: "warning",
        okLabel: t("status.cancel_confirm"),
        cancelLabel: t("status.cancel"),
      });
      if (confirmed) {
        onCancel();
      } else {
        await appWindow.setAlwaysOnTop(true);
      }
    };

    return (
      <div data-tauri-drag-region className="flex items-center h-full bg-surface overflow-hidden select-none">
        <button
          onClick={handleCancel}
          className="h-full border-none cursor-pointer font-semibold"
          style={{
            fontSize: "0.6rem",
            width: "38%",
            backgroundColor: "var(--color-surface-container-highest)",
            color: "#fff",
          }}
        >
          {t("status.cancel")}
        </button>
        <div className="h-full flex items-center justify-center" style={{ width: "24%" }}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" className="opacity-30 pointer-events-none">
            <path d="M12 2l-4 4h3v4H7V7l-4 4 4 4v-3h4v4H8l4 4 4-4h-3v-4h4v3l4-4-4-4v3h-4V6h3z" />
          </svg>
        </div>
        <button
          onClick={onStop}
          className="h-full border-none cursor-pointer font-semibold"
          style={{
            fontSize: "0.6rem",
            width: "38%",
            backgroundColor: "var(--color-tertiary)",
            color: "#fff",
          }}
        >
          {t("status.stop")}
        </button>
      </div>
    );
  }

  // Full-size mode
  const isBusy = status === "processing";
  const displayMessage = (() => {
    if (error) return error;
    if (statusMessage === "no_clicks") return t("status.no_clicks");
    if (statusMessage) return statusMessage;
    if (status === "done") return t("status.done_uploaded");
    return t("status.ready");
  })();
  const isReady = status === "idle" && !error && !statusMessage;

  return (
    <div className="flex flex-col h-full bg-surface">
      {/* Toolbar */}
      <div className="toolbar mx-3 mt-3">
        {email && (
          <>
            <span
              className="text-on-surface-variant mr-auto pl-2"
              style={{ fontSize: "0.6875rem", opacity: 0.8 }}
            >
              {email}
            </span>
            <button onClick={onSignOut} className="icon-btn" title={t("login.sign_out")}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4M16 17l5-5-5-5M21 12H9" />
              </svg>
            </button>
          </>
        )}
        <button onClick={onOpenSettings} className="icon-btn" title={t("settings.title")}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
      </div>

      {/* PII disabled chip */}
      {skipPiiCheck && (
        <div className="flex justify-center pt-2 px-4">
          <button
            onClick={onOpenSettings}
            className="inline-flex items-center gap-1.5 rounded-full px-3 py-1 border cursor-pointer"
            style={{
              fontSize: "0.625rem",
              background: "rgba(255, 180, 50, 0.08)",
              borderColor: "rgba(255, 180, 50, 0.25)",
              color: "rgba(255, 190, 80, 0.85)",
            }}
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
            </svg>
            {t("pii.disabled_chip")}
          </button>
        </div>
      )}

      {/* Center content */}
      <div className="flex-1 flex flex-col items-center justify-center gap-8 px-4">
        {isReady ? (
          <p
            className="text-on-surface-variant"
            style={{ fontSize: "2rem", fontWeight: 700, letterSpacing: "-0.02em" }}
          >
            {t("status.ready")}
          </p>
        ) : (
          <StatusBar message={displayMessage} busy={isBusy} isError={!!error} />
        )}

        <div className="flex flex-col items-center gap-3">
          {status === "done" && (
            <button onClick={onOpenFolder} className="btn-secondary w-56 py-3 text-sm">
              {t("status.open_folder")}
            </button>
          )}
          {status === "error" && (
            <button onClick={onRetry} className="btn-secondary w-56 py-2.5 text-sm">
              {t("status.retry")}
            </button>
          )}
          {(status === "idle" || status === "done" || status === "error" || status === "pii_blocked") && (
            <button
              onClick={onStart}
              className="btn-primary w-56 py-3 text-sm"
              style={{ animation: isReady ? "cta-breathe 3s ease-in-out infinite" : "none" }}
            >
              {t("status.start")}
            </button>
          )}
        </div>
      </div>

      {/* Version */}
      <div className="px-4 pb-3 text-right">
        <span className="text-on-surface-variant" style={{ fontSize: "0.625rem", opacity: 0.5 }}>
          v{version}
        </span>
      </div>

      {/* PII blocked modal */}
      {status === "pii_blocked" && (
        <PiiBlockedModal findings={_piiFindings as never} onDismiss={onDismissPii} />
      )}
    </div>
  );
}
