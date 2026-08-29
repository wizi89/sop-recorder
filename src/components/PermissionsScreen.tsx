import { useTranslation } from "../hooks/useTranslation";
import { openPrivacySettings } from "../lib/tauri";
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
  /** Fires every OS prompt in one batch. */
  onRequestPermissions: () => void;
  onRestart: () => void;
  /** Dismiss to the recorder, which keeps its own banner for what is missing. */
  onSkip: () => void;
  requesting?: boolean;
}

/// One permission's row: what it is, why the recorder needs it, where it stands.
function PermissionRow({
  title,
  why,
  state,
  pane,
}: {
  title: string;
  why: string;
  state: PermissionState;
  pane: PrivacyPane;
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
  // Only a refusal needs its own way out. While a permission is merely
  // undetermined the grant-all button still raises a real dialog, and a second
  // route to the same place would just be noise.
  const needsSettings = state === "denied";

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
        {needsSettings && (
          <button
            onClick={() => void showPrivacySettings(pane)}
            className="mt-1 border-none cursor-pointer bg-transparent p-0 underline"
            style={{ fontSize: "0.64rem", color: "#2CB5C0" }}
          >
            {t("permissions.open_settings")}
          </button>
        )}
      </div>
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
  onRequestPermissions,
  onRestart,
  onSkip,
  requesting = false,
}: PermissionsScreenProps) {
  const { t } = useTranslation();

  const allGranted =
    micPermission === "granted" &&
    screenRecordingPermission === "granted" &&
    accessibilityPermission === "granted";

  const anyDenied =
    micPermission === "denied" ||
    screenRecordingPermission === "denied" ||
    accessibilityPermission === "denied";

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
          />
          <PermissionRow
            title={t("permissions.screen_title")}
            why={t("permissions.screen_why")}
            state={screenRecordingPermission}
            pane="screen"
          />
          <PermissionRow
            title={t("permissions.accessibility_title")}
            why={t("permissions.accessibility_why")}
            state={accessibilityPermission}
            pane="accessibility"
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
            {/* A refusal is the case where the button above cannot help, so
                say why rather than let it look broken. */}
            {anyDenied
              ? t("permissions.denied_hint")
              : t("permissions.settings_hint")}
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
            <button
              onClick={onRequestPermissions}
              disabled={requesting}
              className="rounded-lg border-none font-semibold py-2"
              style={{
                fontSize: "0.75rem",
                background: "#2CB5C0",
                color: "#0B1416",
                opacity: requesting ? 0.6 : 1,
                cursor: requesting ? "wait" : "pointer",
              }}
            >
              {t("permissions.grant_all")}
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
