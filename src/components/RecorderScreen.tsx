import { ask } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "../hooks/useTranslation";
import { StatusBar } from "./StatusBar";
import { PiiBlockedModal } from "./PiiBlockedModal";
import { RateLimitModal } from "./RateLimitModal";
import { ReviewScreen } from "./ReviewScreen";
import { useCaptureCount } from "../hooks/useCaptureCount";
import { useElapsedTime, formatElapsed } from "../hooks/useElapsedTime";
import { useAudioLevel } from "../hooks/useAudioLevel";
import type { RecorderStatus } from "../hooks/useRecorder";
import type { RateLimitInfo } from "../lib/serverErrors";
import type { Quota, MicPermissionState } from "../lib/tauri";

interface RecorderScreenProps {
  email: string | null;
  status: RecorderStatus;
  statusMessage: string;
  error: string | null;
  piiFindings?: unknown | null;
  rateLimit?: RateLimitInfo | null;
  quota?: Quota | null;
  outputDir: string | null;
  skipPiiCheck?: boolean;
  micPermission?: MicPermissionState;
  onStart: () => void;
  onStop: () => void;
  onCancel: () => void;
  onSignOut: () => void;
  onOpenSettings: () => void;
  onOpenFolder: () => void;
  onRetry: () => void;
  onDismissPii: () => void;
  onDismissRateLimit: () => void;
  onUndoLastScreenshot: () => void;
  onConfirmGeneration: () => void;
  onCancelFromReview: () => void;
  onUpgradeQuota?: () => void;
  version: string;
}

export function RecorderScreen({
  email,
  status,
  statusMessage,
  error,
  piiFindings: _piiFindings,
  rateLimit,
  quota,
  outputDir,
  skipPiiCheck,
  micPermission,
  onStart,
  onStop,
  onCancel,
  onSignOut,
  onOpenSettings,
  onOpenFolder,
  onRetry,
  onDismissPii,
  onDismissRateLimit,
  onUndoLastScreenshot,
  onConfirmGeneration,
  onCancelFromReview,
  onUpgradeQuota,
  version,
}: RecorderScreenProps) {
  const { t } = useTranslation();
  const isRecording = status === "recording";
  const captureCount = useCaptureCount(isRecording);
  const elapsedSec = useElapsedTime(isRecording);
  const audioLevel = useAudioLevel();

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

    // Live telemetry strings for the compact bar.
    // VU meter: render a small fixed-width bar whose fill width is the peak
    // level clamped to [0..1]. Green below ~0.8, amber toward clipping.
    const vuFillPct = Math.round(Math.min(1, Math.max(0, audioLevel)) * 100);
    const vuColor = audioLevel > 0.8 ? "#FBBF24" : "#34D399";
    const undoDisabled = captureCount === 0;

    return (
      <div data-tauri-drag-region className="flex items-center h-full bg-surface overflow-hidden select-none">
        <button
          onClick={handleCancel}
          className="h-full border-none cursor-pointer font-semibold"
          style={{
            fontSize: "0.6rem",
            width: "32%",
            backgroundColor: "var(--color-error)",
            color: "#fff",
          }}
        >
          {t("status.cancel")}
        </button>
        <button
          onClick={onUndoLastScreenshot}
          disabled={undoDisabled}
          title={t("status.undo_last")}
          aria-label={t("status.undo_last")}
          className="h-full border-none font-semibold"
          style={{
            width: "13%",
            backgroundColor: "var(--color-surface-container-highest)",
            color: "#fff",
            cursor: undoDisabled ? "not-allowed" : "pointer",
            opacity: undoDisabled ? 0.35 : 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          {/* Undo arrow icon */}
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 7v6h6" />
            <path d="M21 17a9 9 0 00-15-6.7L3 13" />
          </svg>
        </button>
        <div
          className="h-full flex flex-col items-center justify-center pointer-events-none"
          style={{ width: "22%", color: "#fff" }}
        >
          <span style={{ fontSize: "0.55rem", fontWeight: 600, lineHeight: 1.1 }}>
            {captureCount} · {formatElapsed(elapsedSec)}
          </span>
          <div
            aria-label="audio level"
            style={{
              width: "80%",
              height: "3px",
              marginTop: "3px",
              borderRadius: "2px",
              background: "rgba(255,255,255,0.12)",
              overflow: "hidden",
            }}
          >
            <div
              style={{
                width: `${vuFillPct}%`,
                height: "100%",
                background: vuColor,
                transition: "width 60ms linear",
              }}
            />
          </div>
        </div>
        <button
          onClick={onStop}
          className="h-full border-none cursor-pointer font-semibold"
          style={{
            fontSize: "0.6rem",
            width: "33%",
            backgroundColor: "var(--color-primary)",
            color: "#fff",
          }}
        >
          {t("status.stop")}
        </button>
      </div>
    );
  }

  // Review mode: user has stopped recording and is inspecting captured
  // screenshots before committing to a generation.
  if (status === "review" && outputDir) {
    return (
      <ReviewScreen
        outputDir={outputDir}
        captureCount={captureCount}
        elapsedSec={elapsedSec}
        onConfirm={onConfirmGeneration}
        onCancel={onCancelFromReview}
      />
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

  // Quota chip: shown on idle/done/error/pii_blocked/rate_limited screens.
  // The compact recording mode returns early above, so by the time we reach
  // the toolbar we are already guaranteed to be out of `recording` status.
  const showQuotaChip = !!quota;
  const quotaIsWarning = !!quota && quota.remaining <= 1;
  const quotaChipStyle = quotaIsWarning
    ? {
        fontSize: "0.625rem",
        background: "rgba(220, 60, 60, 0.12)",
        borderColor: "rgba(220, 60, 60, 0.35)",
        color: "rgba(255, 130, 130, 0.95)",
      }
    : {
        fontSize: "0.625rem",
        background: "rgba(255, 255, 255, 0.05)",
        borderColor: "rgba(255, 255, 255, 0.12)",
        color: "rgba(255, 255, 255, 0.7)",
      };

  return (
    <div className="flex flex-col h-full bg-surface">
      {/* Toolbar */}
      <div className="toolbar mx-3 mt-3">
        {email && (
          <>
            <span
              className="mr-auto pl-2"
              style={{ fontSize: "0.6875rem", color: "#C5CDD2" }}
            >
              {email}
            </span>
            {showQuotaChip && quota && (
              <span
                className="inline-flex items-center rounded-full px-2 py-0.5 border mr-1"
                title={t("quota.used", { count: quota.count, limit: quota.limit })}
                style={quotaChipStyle}
              >
                {t("quota.used", { count: quota.count, limit: quota.limit })}
              </span>
            )}
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

      {/* Microphone permission warning chip (shown only when denied) */}
      {micPermission === "denied" && (
        <div className="flex justify-center pt-2 px-4">
          <div
            className="inline-flex items-center gap-1.5 rounded-full px-3 py-1 border"
            style={{
              fontSize: "0.625rem",
              background: "rgba(220, 60, 60, 0.12)",
              borderColor: "rgba(220, 60, 60, 0.35)",
              color: "rgba(255, 130, 130, 0.95)",
            }}
          >
            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
            </svg>
            {t("mic.permission_denied")}
          </div>
        </div>
      )}

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
          {/* Retry-from-disk: visible whenever a preserved session dir exists
              in an idle-ish state, so the user never loses a captured recording
              after a transient failure (network, rate limit, server 5xx). */}
          {outputDir && (status === "error" || status === "idle") && (
            <button onClick={onRetry} className="btn-secondary w-56 py-2.5 text-sm">
              {t("status.retry_from_disk")}
            </button>
          )}
          {(status === "idle" || status === "done" || status === "error" || status === "pii_blocked" || status === "rate_limited") && (
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
        <span style={{ fontSize: "0.625rem", color: "#6B7780" }}>
          v{version}
        </span>
      </div>

      {/* PII blocked modal */}
      {status === "pii_blocked" && (
        <PiiBlockedModal findings={_piiFindings as never} onDismiss={onDismissPii} />
      )}

      {/* Rate limit modal */}
      {status === "rate_limited" && (
        <RateLimitModal
          count={rateLimit?.count ?? null}
          limit={rateLimit?.limit ?? null}
          onDismiss={onDismissRateLimit}
          onUpgrade={onUpgradeQuota}
        />
      )}
    </div>
  );
}
