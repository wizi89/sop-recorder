import { useTranslation } from "../hooks/useTranslation";
import { StatusBar } from "./StatusBar";
import { PiiBlockedModal } from "./PiiBlockedModal";
import { RateLimitModal } from "./RateLimitModal";
import { ReviewScreen } from "./ReviewScreen";
import { useCaptureCount } from "../hooks/useCaptureCount";
import { useElapsedTime } from "../hooks/useElapsedTime";
import type { RecorderStatus } from "../hooks/useRecorder";
import type { RateLimitInfo } from "../lib/serverErrors";
import type {
  Quota,
  MicPermissionState,
  ScreenRecordingPermissionState,
  AccessibilityPermissionState,
} from "../lib/tauri";

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
  screenRecordingPermission?: ScreenRecordingPermissionState;
  accessibilityPermission?: AccessibilityPermissionState;
  onRequestPermissions?: () => void;
  onStart: () => void;
  onSignOut: () => void;
  onOpenSettings: () => void;
  onOpenFolder: () => void;
  onRetry: () => void;
  onDismissPii: () => void;
  onDismissRateLimit: () => void;
  onConfirmGeneration: () => void;
  onCancelFromReview: () => void;
  onUpgradeQuota?: () => void;
  /** When provided, renders a secondary button to run generation against a
   *  picked recording folder (regenerate an existing recording). */
  onGenerateFromFolder?: () => void;
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
  screenRecordingPermission,
  accessibilityPermission,
  onRequestPermissions,
  onStart,
  onSignOut,
  onOpenSettings,
  onOpenFolder,
  onRetry,
  onDismissPii,
  onDismissRateLimit,
  onConfirmGeneration,
  onCancelFromReview,
  onUpgradeQuota,
  onGenerateFromFolder,
  version,
}: RecorderScreenProps) {
  const { t } = useTranslation();
  const isRecording = status === "recording";
  const captureCount = useCaptureCount(isRecording);
  const elapsedSec = useElapsedTime(isRecording);

  // Recording: the controls live in the bar window, and this window is hidden
  // for the duration. It still needs an honest state for the moments it is
  // not -- falling through to the idle screen would offer a Start button for a
  // recording that is already running.
  if (status === "recording") {
    return (
      <div className="flex items-center justify-center h-full p-6 text-center bg-surface">
        <p className="text-sm text-on-surface-variant">
          {t("status.recording_in_progress")}
        </p>
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
  // Every permission the recorder is missing, one line each. A fused
  // sentence per combination needed a new string for every pair and did not
  // survive a third permission being added.
  const deniedPermissions = [
    micPermission === "denied" ? t("mic.permission_denied") : null,
    screenRecordingPermission === "denied"
      ? t("permissions.screen_recording_denied")
      : null,
    accessibilityPermission === "denied"
      ? t("permissions.accessibility_denied")
      : null,
  ].filter((m): m is string => m !== null);
  const permissionsBlocked = deniedPermissions.length > 0;

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

      {/* Permission banner: shown when any recording permission is missing.
          Single CTA triggers the OS prompts via the Rust bootstrap command,
          so the user grants everything in one sitting instead of being
          interrupted by a fresh prompt at every recording start. */}
      {permissionsBlocked && (
        <div className="flex justify-center pt-2 px-4">
          <div
            className="flex items-center gap-2 rounded-lg px-3 py-2 border w-full"
            style={{
              fontSize: "0.7rem",
              background: "rgba(220, 60, 60, 0.10)",
              borderColor: "rgba(220, 60, 60, 0.35)",
              color: "rgba(255, 180, 180, 0.95)",
            }}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
              <path d="M12 9v4M12 17h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
            </svg>
            <div className="flex-1 leading-tight">
              {deniedPermissions.map((message) => (
                <div key={message}>{message}</div>
              ))}
            </div>
            {onRequestPermissions && (
              <button
                onClick={onRequestPermissions}
                className="rounded px-2 py-1 border-none cursor-pointer font-medium"
                style={{
                  fontSize: "0.65rem",
                  background: "rgba(255, 180, 180, 0.18)",
                  color: "rgba(255, 220, 220, 0.95)",
                }}
              >
                {t("permissions.grant")}
              </button>
            )}
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
              disabled={permissionsBlocked}
              title={permissionsBlocked ? t("permissions.grant_to_start") : undefined}
              className="btn-primary w-56 py-3 text-sm"
              style={{
                animation: isReady && !permissionsBlocked ? "cta-breathe 3s ease-in-out infinite" : "none",
                opacity: permissionsBlocked ? 0.5 : 1,
                cursor: permissionsBlocked ? "not-allowed" : "pointer",
              }}
            >
              {t("status.start")}
            </button>
          )}
          {/* Secondary action: generate against an existing recording folder,
              skipping the record step. The primary CTA above always stays
              "Aufnahme starten". Reuses the selected model/pipeline. */}
          {onGenerateFromFolder &&
            (status === "idle" || status === "done" || status === "error") && (
              <button
                onClick={onGenerateFromFolder}
                className="btn-secondary w-56 py-3 text-sm"
              >
                {t("status.generate_from_folder")}
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
