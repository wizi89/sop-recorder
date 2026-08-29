import { ask } from "@tauri-apps/plugin-dialog";
import { emit } from "@tauri-apps/api/event";
import { useTranslation } from "../hooks/useTranslation";
import { useCaptureCount } from "../hooks/useCaptureCount";
import { useElapsedTime, formatElapsed } from "../hooks/useElapsedTime";
import { useAudioLevel } from "../hooks/useAudioLevel";
import { useAudioSilent } from "../hooks/useAudioSilent";
import { useRecordingActive } from "../hooks/useRecordingActive";

/**
 * The compact recording bar, which lives in a window of its own.
 *
 * Separate from the main window because of a macOS rule with no way around it:
 * whether a window may appear inside another app's fullscreen Space is fixed
 * when the window is created, from the app's activation policy at that moment.
 * The main window is a normal document window and can never gain that; the bar
 * is built under a momentary accessory policy and can. See `BAR_LABEL` in
 * `commands/window.rs` for the measurements.
 *
 * It owns no recording state. Telemetry arrives as events the Rust side already
 * broadcasts to every window, and the three controls emit events that the main
 * window -- still running, just hidden -- turns into the same actions it always
 * performed. Keeping the orchestration in one window is what stops the two
 * webviews from disagreeing about what the recorder is doing.
 */
export function RecordingBar() {
  const { t } = useTranslation();
  // Not a constant `true`: this window outlives every recording it shows, so
  // the hooks need the session boundary or they carry one recording's state
  // into the next.
  const recording = useRecordingActive();
  const captureCount = useCaptureCount(recording);
  const elapsedSec = useElapsedTime(recording);
  const audioLevel = useAudioLevel();
  const audioSilent = useAudioSilent(recording);

  const handleCancel = async () => {
    const confirmed = await ask(t("status.cancel_message"), {
      title: t("status.cancel_title"),
      kind: "warning",
      okLabel: t("status.cancel_confirm"),
      cancelLabel: t("status.cancel"),
    });
    if (confirmed) await emit("bar:cancel");
  };

  // VU meter: fill width is the peak level clamped to [0..1].
  // Green below ~0.8, amber toward clipping.
  const vuFillPct = Math.round(Math.min(1, Math.max(0, audioLevel)) * 100);
  const vuColor = audioLevel > 0.8 ? "#FBBF24" : "#34D399";
  const undoDisabled = captureCount === 0;

  // The bar is deliberately NOT a drag region. It anchors itself to the corner
  // of the work area for the whole recording; making it draggable invited the
  // user to move it mid-recording, and the press that started the drag was
  // captured as the recording's first step.
  return (
    <div className="flex items-center h-full bg-surface overflow-hidden select-none">
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
        onClick={() => void emit("bar:undo")}
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
        title={audioSilent ? t("status.no_audio_hint") : undefined}
      >
        <span
          style={{
            fontSize: "0.55rem",
            fontWeight: 600,
            lineHeight: 1.1,
            color: audioSilent ? "var(--color-error)" : undefined,
          }}
        >
          {audioSilent
            ? t("status.no_audio")
            : `${captureCount} · ${formatElapsed(elapsedSec)}`}
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
              width: audioSilent ? "100%" : `${vuFillPct}%`,
              height: "100%",
              background: audioSilent ? "var(--color-error)" : vuColor,
              transition: "width 60ms linear",
            }}
          />
        </div>
      </div>
      <button
        onClick={() => void emit("bar:stop")}
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
