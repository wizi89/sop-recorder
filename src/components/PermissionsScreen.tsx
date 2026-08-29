import { useCallback, useState } from "react";
import { useTranslation } from "../hooks/useTranslation";
import { openPrivacySettings, requestPermission } from "../lib/tauri";
import type {
  MicPermissionState,
  ScreenRecordingPermissionState,
  AccessibilityPermissionState,
} from "../lib/tauri";

type PermissionState = "granted" | "denied" | "undetermined" | "unknown";

/**
 * The permission panes a refused grant can be restored from.
 *
 * Needed because "ask the user" stops working the moment they have said no
 * once: macOS shows a permission dialog only while the status is
 * undetermined, so for a refused permission the only honest thing the app can
 * do is take them to the switch.
 */
type PrivacyPane = "microphone" | "screen" | "accessibility";

async function showPrivacySettings(pane: PrivacyPane) {
  try {
    await openPrivacySettings(pane);
  } catch (e) {
    console.warn("Could not open System Settings:", e);
  }
}

interface PermissionsScreenProps {
  micPermission: MicPermissionState;
  screenRecordingPermission: ScreenRecordingPermissionState;
  accessibilityPermission: AccessibilityPermissionState;
  onRestart: () => void;
  /** Dismiss to the recorder, which keeps its own banner for what is missing. */
  onSkip: () => void;
}

/// One permission's row: what it is, why the recorder needs it, where it stands.
function PermissionRow({
  title,
  why,
  state,
  pane,
  grantableInDialog,
  needsRestart = false,
  mayNeedManualAdd = false,
  onAsk,
  busy,
}: {
  title: string;
  why: string;
  state: PermissionState;
  pane: PrivacyPane;
  /** Whether an OS dialog can grant this outright, or only System Settings can. */
  grantableInDialog: boolean;
  /** Whether the OS only reports this grant to a freshly started process. */
  needsRestart?: boolean;
  /** Whether macOS may fail to list the app in its pane, leaving only `+`. */
  mayNeedManualAdd?: boolean;
  onAsk: (pane: PrivacyPane) => void;
  busy: boolean;
}) {
  const { t } = useTranslation();
  const granted = state === "granted";
  const label = granted
    ? t("permissions.state_granted")
    : state === "denied"
      ? t("permissions.state_denied")
      : state === "undetermined"
        ? t("permissions.state_undetermined")
        : t("permissions.state_unknown");
  // Every row that is not granted carries its own action, because a single
  // button at the bottom could only ever describe one of them.
  //
  // "Erteilen" is offered only where a dialog can actually hand over the
  // grant, which is the microphone alone: Screen Recording and Accessibility
  // are switches in System Settings, and their prompts do no more than open
  // that pane. Sending the user straight there is the same journey with one
  // fewer dialog in the way -- and, for a microphone already refused, the only
  // journey that exists.
  const canAsk = grantableInDialog && (state === "undetermined" || state === "unknown");

  return (
    <div className="flex gap-3 items-start">
      <div
        aria-hidden
        className="flex items-center justify-center rounded-full"
        style={{
          width: 18,
          height: 18,
          marginTop: 2,
          flexShrink: 0,
          background: granted ? "rgba(44, 181, 192, 0.18)" : "rgba(220, 60, 60, 0.14)",
          color: granted ? "#2CB5C0" : "rgba(255, 150, 150, 0.95)",
          fontSize: "0.6rem",
        }}
      >
        {granted ? "✓" : "!"}
      </div>
      <div className="flex-1 leading-snug">
        <div className="flex items-baseline gap-2">
          <span style={{ fontSize: "0.78rem", fontWeight: 600 }}>{title}</span>
          <span
            style={{
              fontSize: "0.62rem",
              color: granted ? "#2CB5C0" : "rgba(255, 150, 150, 0.95)",
            }}
          >
            {label}
          </span>
        </div>
        <div style={{ fontSize: "0.68rem", color: "#A8B2B8" }}>{why}</div>
        {!granted && needsRestart && (
          <div style={{ fontSize: "0.62rem", color: "#8C979D", marginTop: 2 }}>
            {t("permissions.needs_restart")}
          </div>
        )}
        {/* Measured on macOS 26, with a properly signed build: CogniClone does
            not appear in that pane by itself. CGRequestScreenCaptureAccess
            returns, the pane opens, and there is no row. Signing was the
            leading suspect and has been ruled out -- a certificate changed
            nothing here. So this is the normal path rather than a fallback,
            and the wording says so. */}
        {!granted && mayNeedManualAdd && (
          <div style={{ fontSize: "0.62rem", color: "#8C979D", marginTop: 2 }}>
            {t("permissions.add_manually")}
          </div>
        )}
      </div>
      {!granted && (
        <button
          onClick={() =>
            canAsk ? void onAsk(pane) : void showPrivacySettings(pane)
          }
          disabled={busy}
          className="rounded-md border-none font-semibold self-center shrink-0"
          style={{
            fontSize: "0.62rem",
            padding: "4px 8px",
            whiteSpace: "nowrap",
            background: canAsk ? "#2CB5C0" : "rgba(255,255,255,0.10)",
            color: canAsk ? "#0B1416" : "#D6DEE2",
            opacity: busy ? 0.6 : 1,
            cursor: busy ? "wait" : "pointer",
          }}
        >
          {canAsk ? t("permissions.grant_one") : t("permissions.open_settings")}
        </button>
      )}
    </div>
  );
}

/**
 * First-run permission setup.
 *
 * Shown instead of the recorder while anything is missing, so the three grants
 * happen in one sitting with a reason attached, rather than one interruption
 * per recording. Dismissible on purpose: the microphone state is a probe rather
 * than a real permission query, so a wrong "missing" must never be able to lock
 * anyone out of their own app -- the recorder's banner carries the warning on.
 */
export function PermissionsScreen({
  micPermission,
  screenRecordingPermission,
  accessibilityPermission,
  onRestart,
  onSkip,
}: PermissionsScreenProps) {
  const { t } = useTranslation();

  const allGranted =
    micPermission === "granted" &&
    screenRecordingPermission === "granted" &&
    accessibilityPermission === "granted";

  const [askingPane, setAskingPane] = useState<PrivacyPane | null>(null);
  const askOne = useCallback(async (pane: PrivacyPane) => {
    setAskingPane(pane);
    try {
      await requestPermission(pane);
    } catch (e) {
      console.warn("Permission request failed:", e);
    } finally {
      setAskingPane(null);
    }
  }, []);

  return (
    <div className="flex flex-col h-full bg-surface overflow-y-auto">
      <div className="flex-1 flex flex-col justify-center gap-4 px-6 py-5">
        <div>
          <div style={{ fontSize: "0.95rem", fontWeight: 600 }}>
            {t("permissions.setup_title")}
          </div>
          <div
            className="mt-1 leading-snug"
            style={{ fontSize: "0.7rem", color: "#A8B2B8" }}
          >
            {t("permissions.setup_intro")}
          </div>
        </div>

        <div className="flex flex-col gap-3">
          <PermissionRow
            title={t("permissions.mic_title")}
            why={t("permissions.mic_why")}
            state={micPermission}
            pane="microphone"
            grantableInDialog
            onAsk={askOne}
            busy={askingPane === "microphone"}
          />
          <PermissionRow
            title={t("permissions.screen_title")}
            why={t("permissions.screen_why")}
            state={screenRecordingPermission}
            pane="screen"
            grantableInDialog={false}
            needsRestart
            mayNeedManualAdd
            onAsk={askOne}
            busy={askingPane === "screen"}
          />
          <PermissionRow
            title={t("permissions.accessibility_title")}
            why={t("permissions.accessibility_why")}
            state={accessibilityPermission}
            pane="accessibility"
            grantableInDialog={false}
            onAsk={askOne}
            busy={askingPane === "accessibility"}
          />
        </div>

        {allGranted ? (
          <div
            className="rounded-lg px-3 py-2 leading-snug"
            style={{
              fontSize: "0.68rem",
              background: "rgba(44, 181, 192, 0.10)",
              border: "1px solid rgba(44, 181, 192, 0.30)",
              color: "#A8B2B8",
            }}
          >
            <div style={{ color: "#2CB5C0", fontWeight: 600 }}>
              {t("permissions.all_granted")}
            </div>
            <div className="mt-1">{t("permissions.restart_hint")}</div>
          </div>
        ) : (
          <div
            className="leading-snug"
            style={{ fontSize: "0.65rem", color: "#A8B2B8" }}
          >
            {t("permissions.settings_hint")}
          </div>
        )}

        <div className="flex flex-col gap-2">
          {allGranted ? (
            <button
              onClick={onRestart}
              className="rounded-lg border-none cursor-pointer font-semibold py-2"
              style={{ fontSize: "0.75rem", background: "#2CB5C0", color: "#0B1416" }}
            >
              {t("permissions.restart")}
            </button>
          ) : (
            // Always reachable, not only once everything is green. macOS
            // reports a Screen Recording grant to a fresh process only, so
            // after granting it in System Settings the row stays red and the
            // user has no way to make the app look again -- which is exactly
            // where this screen used to strand them.
            <button
              onClick={onRestart}
              className="rounded-lg cursor-pointer font-semibold py-2"
              style={{
                fontSize: "0.72rem",
                background: "transparent",
                border: "1px solid rgba(255,255,255,0.18)",
                color: "#D6DEE2",
              }}
            >
              {t("permissions.restart_now")}
            </button>
          )}
          <button
            onClick={onSkip}
            className="border-none cursor-pointer bg-transparent"
            style={{ fontSize: "0.66rem", color: "#A8B2B8" }}
          >
            {t("permissions.skip")}
          </button>
        </div>
      </div>
    </div>
  );
}
