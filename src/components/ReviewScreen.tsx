import { useEffect, useState } from "react";
import { useTranslation } from "../hooks/useTranslation";
import {
  listCaptureRecords,
  listSessionScreenshots,
  readScreenshotBytes,
  type StepCaptureRecord,
} from "../lib/tauri";
import { formatElapsed } from "../hooks/useElapsedTime";
import { usePipelines } from "../hooks/usePipelines";

interface ReviewScreenProps {
  outputDir: string;
  /** Count captured during the recording (from the live telemetry hook). */
  captureCount: number;
  /**
   * Elapsed recording duration in whole seconds at the moment Stop was clicked.
   * Zero when reviewing a folder picked from disk, which has no live timing --
   * the summary then reports the count alone rather than "00:00 Min".
   */
  elapsedSec: number;
  /**
   * Captures that failed during this recording. Non-zero means the user did
   * more than the guide will show, so it is said plainly here rather than left
   * for them to notice a missing step later.
   */
  failedCaptures?: number;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * A step the user has to look at before generating (privacy-capture D7): the
 * password check may have missed a field, the active window could not be
 * determined, or the step carries no record of how it was captured.
 */
function needsReview(record: StepCaptureRecord | undefined): boolean {
  if (!record) return true;
  return (
    record.password_check === "flagged" ||
    record.password_check === "unknown" ||
    record.capture_scope === "monitor_fallback" ||
    record.capture_scope === "unknown"
  );
}

/**
 * Post-recording, pre-generation review screen.
 *
 * Shows a summary (count + elapsed) and a horizontal thumbnail strip of
 * captured screenshots. The user confirms to invoke generation or cancels
 * to discard and return to idle. When the session has zero screenshots the
 * parent should not render this screen at all -- the hook skips review and
 * transitions to idle with `no_clicks` status instead.
 *
 * The pipeline selector lives here rather than on the idle screen: at review
 * the user has seen what they actually captured, so "what am I recording" is
 * answered with knowledge rather than intent, and the control sits next to the
 * action that consumes it. The idle screen is one button and stays that way.
 * This also covers regenerating from an existing recording folder, which has no
 * record-time moment to choose at.
 *
 * It is a separate control from `pipeline_version` and the model picker, which
 * are engineering knobs gated per org in the Settings window. Those answer
 * "which prompt architecture"; only this one asks a question a user can answer.
 */
export function ReviewScreen({
  outputDir,
  captureCount,
  elapsedSec,
  failedCaptures = 0,
  onConfirm,
  onCancel,
}: ReviewScreenProps) {
  const { t } = useTranslation();
  const [thumbs, setThumbs] = useState<Array<{ path: string; url: string; marked: boolean }>>(
    [],
  );
  const [reviewed, setReviewed] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { pipelines, selectedId, select, visible: showPipelines } = usePipelines();
  const selectedPipeline = pipelines.find((p) => p.id === selectedId);

  useEffect(() => {
    let cancelled = false;
    const createdUrls: string[] = [];

    (async () => {
      try {
        const [paths, records] = await Promise.all([
          listSessionScreenshots(outputDir),
          // A failed read marks every step: no record is never read as safe.
          listCaptureRecords(outputDir).catch((): StepCaptureRecord[] => []),
        ]);
        // Read each file's bytes and wrap in a Blob URL for display.
        const entries = await Promise.all(
          paths.map(async (p) => {
            const bytes = await readScreenshotBytes(p);
            const blob = new Blob([bytes], { type: "image/png" });
            const url = URL.createObjectURL(blob);
            createdUrls.push(url);
            return { path: p, url, marked: needsReview(records.find((r) => r.path === p)) };
          }),
        );
        if (cancelled) {
          createdUrls.forEach((u) => URL.revokeObjectURL(u));
          return;
        }
        setThumbs(entries);
        setLoading(false);
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
        setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
      // Revoke blob URLs to free memory on unmount.
      createdUrls.forEach((u) => URL.revokeObjectURL(u));
    };
  }, [outputDir]);

  const markedCount = thumbs.filter((thumb) => thumb.marked).length;
  const blocked = loading || thumbs.length === 0 || (markedCount > 0 && !reviewed);

  return (
    <div className="flex flex-col h-full bg-surface">
      <div className="px-4 pt-4 pb-2">
        <p className="text-on-surface text-sm font-semibold">
          {t("review.title")}
        </p>
        <p className="text-on-surface-variant mt-1" style={{ fontSize: "0.75rem" }}>
          {/* Once thumbnails load they are the ground truth for the count; a
              folder picked from disk has no live capture telemetry at all. */}
          {elapsedSec > 0
            ? t("review.summary", {
                count: loading ? captureCount : thumbs.length,
                elapsed: formatElapsed(elapsedSec),
              })
            : t("review.summary_count_only", {
                count: loading ? captureCount : thumbs.length,
              })}
        </p>
        {failedCaptures > 0 && (
          <p
            className="mt-2 rounded px-2 py-1.5 leading-snug"
            style={{
              fontSize: "0.6875rem",
              background: "rgba(255, 180, 50, 0.06)",
              border: "1px solid rgba(255, 180, 50, 0.15)",
              color: "rgba(255, 190, 80, 0.9)",
            }}
          >
            {t("review.failed_captures", {
              count: failedCaptures,
              total: (loading ? captureCount : thumbs.length) + failedCaptures,
            })}
          </p>
        )}
      </div>

      {/* Pipeline selector. Renders only at two or more entries: a dropdown
          with one option is not a choice, and zero entries is the designed
          invisible state for an installation with nothing configured.

          Sits above the thumbnail strip, not above the buttons: a native
          <select> opens downward from wherever it is, and at the bottom of the
          window the option list spilled past the window edge. Same row shape
          and classes as the settings dropdowns, so the two read as one control
          vocabulary. */}
      {showPipelines && (
        <div className="px-4 pt-2 pb-5">
          <label htmlFor="pipeline-select" className="label-sm">
            {t("review.pipeline_label")}
          </label>
          <select
            id="pipeline-select"
            value={selectedId}
            onChange={(e) => select(e.target.value)}
            className="bg-surface-container-highest text-on-background mt-2 w-full rounded-lg px-3 py-2 text-sm outline-none"
          >
            <option value="">{t("review.pipeline_default")}</option>
            {pipelines.map((p) => (
              <option key={p.id} value={p.id}>
                {p.display_name}
              </option>
            ))}
          </select>
          {/* The default entry gets a description too. It is the one option
              that is not a pipeline, so without one it reads as a mystery
              setting sitting among named ones. */}
          <p
            className="text-on-surface-variant mt-2"
            style={{ fontSize: "0.6875rem" }}
          >
            {selectedPipeline
              ? selectedPipeline.description
              : t("review.pipeline_default_description")}
          </p>
        </div>
      )}

      {/* Thumbnail strip */}
      <div className="flex-1 min-h-0 px-4 overflow-y-auto">
        {loading && (
          <p className="text-on-surface-variant text-xs">{t("review.loading")}</p>
        )}
        {error && (
          <p className="text-on-surface-variant text-xs" style={{ color: "#F87171" }}>
            {error}
          </p>
        )}
        {!loading && !error && (
          <div
            className="flex gap-2 overflow-x-auto pb-2"
            style={{ scrollbarWidth: "thin" }}
          >
            {thumbs.map((thumb, idx) => (
              <div
                key={thumb.path}
                className="flex-shrink-0 rounded border"
                style={{
                  width: 96,
                  borderColor: "rgba(255,255,255,0.12)",
                  background: "rgba(255,255,255,0.04)",
                }}
              >
                <img
                  src={thumb.url}
                  alt={`step ${idx + 1}`}
                  style={{
                    width: "100%",
                    height: 54,
                    objectFit: "cover",
                    borderRadius: "3px 3px 0 0",
                  }}
                />
                <div
                  className="text-on-surface-variant text-center"
                  style={{ fontSize: "0.625rem", padding: "2px 0" }}
                >
                  {t("review.step_label", { n: idx + 1 })}
                </div>
                {thumb.marked && (
                  <div
                    className="text-center"
                    style={{ fontSize: "0.625rem", color: "rgba(255, 190, 80, 0.9)" }}
                  >
                    {t("review.marked_badge")}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {markedCount > 0 && (
        <div className="px-4 pt-2 flex flex-col gap-1.5">
          <p className="text-on-surface-variant leading-snug" style={{ fontSize: "0.6875rem" }}>
            {t("review.marked_hint", { count: markedCount })}
          </p>
          <label className="flex items-center gap-2 text-xs">
            <input
              type="checkbox"
              checked={reviewed}
              onChange={(e) => setReviewed(e.target.checked)}
            />
            {t("review.marked_confirm")}
          </label>
        </div>
      )}

      {/* Action buttons */}
      <div className="flex gap-2 px-4 pb-3 pt-2">
        <button
          onClick={onCancel}
          className="btn-secondary flex-1 py-2 text-xs"
        >
          {t("review.cancel")}
        </button>
        <button
          onClick={onConfirm}
          className="btn-primary flex-1 py-2 text-xs"
          disabled={blocked}
          style={{
            opacity: blocked ? 0.4 : 1,
            cursor: blocked ? "not-allowed" : "pointer",
          }}
        >
          {t("review.confirm")}
        </button>
      </div>
    </div>
  );
}
