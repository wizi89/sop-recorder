import { useTranslation } from "../hooks/useTranslation";
import { StatusBar } from "./StatusBar";
import type { RecorderStatus } from "../hooks/useRecorder";

interface RecorderScreenProps {
  email: string | null;
  status: RecorderStatus;
  statusMessage: string;
  error: string | null;
  outputDir: string | null;
  onStart: () => void;
  onStop: () => void;
  onSignOut: () => void;
  onOpenSettings: () => void;
  onOpenFolder: () => void;
  onRetry: () => void;
  version: string;
}

export function RecorderScreen({
  email,
  status,
  statusMessage,
  error,
  onStart,
  onStop,
  onSignOut,
  onOpenSettings,
  onOpenFolder,
  onRetry,
  version,
}: RecorderScreenProps) {
  const { t } = useTranslation();

  // Compact recording mode
  if (status === "recording") {
    return (
      <div className="flex items-center justify-center h-full p-2 bg-surface">
        <button onClick={onStop} className="btn-stop w-full py-2.5 text-sm">
          {t("status.stop")}
        </button>
      </div>
    );
  }

  // Full-size mode
  const isBusy = status === "processing";
  const displayMessage = (() => {
    if (error) return error;
    if (statusMessage) return statusMessage;
    if (status === "done") return t("status.done");
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
          {(status === "idle" || status === "done" || status === "error") && (
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
    </div>
  );
}
