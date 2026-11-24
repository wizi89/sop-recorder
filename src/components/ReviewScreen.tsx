import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "../hooks/useTranslation";
import {
  deleteStep,
  listCaptureRecords,
  listSessionScreenshots,
  loadRedactions,
  loadUnrecorded,
  readScreenshotBytes,
  resetRedactions,
  saveRedactions,
  type RedactionBox,
  type StepCaptureRecord,
} from "../lib/tauri";
import { StepEditor, boxKey, hasBox, type Thumb } from "./StepEditor";
import { ReviewThumb } from "./ReviewThumb";
import { LimboCard, limboApps } from "./Limbo";
import { limbos, type UnrecordedVisit } from "../lib/limbos";
import { formatElapsed } from "../hooks/useElapsedTime";
import { PipelineSelector } from "./PipelineSelector";

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
  /** Why the last upload was refused because of the boxes (D5). */
  uploadError?: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}

type BoxesByStep = Record<number, RedactionBox[]>;

/**
 * A step the user has to look at before generating (privacy-capture D7): the
 * password check may have missed a field, the active window could not be
 * determined, the window list was partial (masking on or off), or the step
 * carries no record of how it was captured.
 */
function needsReview(record: StepCaptureRecord | undefined): boolean {
  if (!record) return true;
  return (
    record.password_check === "flagged" ||
    record.password_check === "unknown" ||
    record.capture_scope === "monitor_fallback" ||
    record.capture_scope === "unknown" ||
    record.window_list_partial
  );
}

function toBoxesByStep(
  steps: Record<string, RedactionBox[]> | undefined,
): BoxesByStep {
  const out: BoxesByStep = {};
  for (const [k, v] of Object.entries(steps ?? {})) out[Number(k)] = v;
  return out;
}

export function ReviewScreen({
  outputDir,
  captureCount,
  elapsedSec,
  failedCaptures = 0,
  uploadError = null,
  onConfirm,
  onCancel,
}: ReviewScreenProps) {
  const { t } = useTranslation();
  const [thumbs, setThumbs] = useState<Thumb[]>([]);
  // Read by the queued operations, which outlive the render that queued them.
  const thumbsRef = useRef<Thumb[]>([]);
  thumbsRef.current = thumbs;
  /** Screenshots when the screen opened; the failed-capture total keeps it (D6). */
  const [openedWith, setOpenedWith] = useState(0);
  const [deletedCount, setDeletedCount] = useState(0);
  const [reviewed, setReviewed] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** When each step was taken, to place the limbos between them. */
  const [times, setTimes] = useState<Record<number, number>>({});
  const [visits, setVisits] = useState<UnrecordedVisit[]>([]);

  const [stored, setStored] = useState<BoxesByStep>({});
  const [desired, setDesired] = useState<BoxesByStep>({});
  // Two box changes before the next render must both land (#58).
  const desiredRef = useRef<BoxesByStep>({});
  desiredRef.current = desired;
  const [failedSaves, setFailedSaves] = useState<Record<number, string>>({});
  const [leftovers, setLeftovers] = useState<number[]>([]);
  const [unreadable, setUnreadable] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  // The refusal came from the last Generate; a later successful delete, reset
  // or resync may have fixed its cause. A new Generate remounts this screen.
  const [refusalCleared, setRefusalCleared] = useState(false);
  const [pending, setPending] = useState(0);
  const queue = useRef<Promise<void>>(Promise.resolve());

  const [openStep, setOpenStep] = useState<number | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const [natural, setNatural] = useState<
    Record<number, { w: number; h: number }>
  >({});

  useEffect(() => {
    let cancelled = false;
    const createdUrls: string[] = [];

    (async () => {
      try {
        // The records list every screenshot, so they are also the list of
        // paths. Only if they cannot be read is the folder listed on its own,
        // and then every step is marked: no record is never read as safe.
        let shots: Array<{
          step: number;
          path: string;
          record?: StepCaptureRecord;
        }>;
        try {
          shots = (await listCaptureRecords(outputDir)).map((record) => ({
            step: record.step,
            path: record.path,
            record,
          }));
        } catch {
          shots = await listSessionScreenshots(outputDir);
        }
        const loaded = await loadRedactions(outputDir);
        // Informational only: a file that cannot be read shows no limbos
        // rather than keeping the user from their steps.
        const unrecorded =
          (await loadUnrecorded(outputDir).catch(() => [])) ?? [];
        // Read each file's bytes and wrap in a Blob URL for display.
        const entries = await Promise.all(
          shots.map(async ({ step, path, record }) => {
            const bytes = await readScreenshotBytes(path);
            const blob = new Blob([bytes], { type: "image/png" });
            const url = URL.createObjectURL(blob);
            createdUrls.push(url);
            return { step, path, url, marked: needsReview(record) };
          }),
        );
        if (cancelled) {
          createdUrls.forEach((u) => URL.revokeObjectURL(u));
          return;
        }
        const boxes = toBoxesByStep(loaded.steps);
        setThumbs(entries);
        setOpenedWith(entries.length);
        setTimes(
          Object.fromEntries(
            shots.map((x) => [x.step, x.record?.timestamp_seconds ?? 0]),
          ),
        );
        setVisits(unrecorded);
        setStored(boxes);
        setDesired(boxes);
        setLeftovers(loaded.leftover_steps ?? []);
        setUnreadable(loaded.unreadable ?? null);
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

  /** Saves, deletes and the reset run one at a time, in order (D5). */
  const enqueue = useCallback((op: () => Promise<void>) => {
    setPending((n) => n + 1);
    queue.current = queue.current
      .then(op)
      .catch(() => {})
      .finally(() => setPending((n) => n - 1));
  }, []);

  const saveStep = useCallback(
    (step: number, list: RedactionBox[]) => {
      enqueue(async () => {
        try {
          const now = await saveRedactions(outputDir, step, list);
          setStored((s) => ({ ...s, [step]: now }));
          setFailedSaves((f) => {
            const { [step]: _, ...rest } = f;
            return rest;
          });
        } catch (e) {
          setFailedSaves((f) => ({ ...f, [step]: String(e) }));
        }
      });
    },
    [enqueue, outputDir],
  );

  /** `change` gets the latest list, not this render's, so no change is lost. */
  const setStepBoxes = (
    step: number,
    change: (list: RedactionBox[]) => RedactionBox[],
  ) => {
    const before = desiredRef.current[step] ?? [];
    const next = change(before);
    if (next === before) return;
    desiredRef.current = { ...desiredRef.current, [step]: next };
    setDesired((d) => ({ ...d, [step]: next }));
    saveStep(step, next);
  };

  /** After a delete failed, the disk decides what is left (D5). */
  const resync = useCallback(async () => {
    try {
      const present = new Set(
        (await listSessionScreenshots(outputDir)).map((s) => s.step),
      );
      const gone = thumbsRef.current.filter((x) => !present.has(x.step)).length;
      setThumbs((ts) => ts.filter((x) => present.has(x.step)));
      setDeletedCount((n) => n + gone);
      const loaded = await loadRedactions(outputDir);
      setLeftovers(loaded.leftover_steps ?? []);
      setUnreadable(loaded.unreadable ?? null);
      setRefusalCleared(true);
    } catch (e) {
      setOpError(String(e));
    }
  }, [outputDir]);

  const removeStep = useCallback(
    (step: number) => {
      enqueue(async () => {
        try {
          await deleteStep(outputDir, step);
          setOpError(null);
          setRefusalCleared(true);
          if (thumbsRef.current.some((x) => x.step === step)) {
            setDeletedCount((n) => n + 1);
          }
          setThumbs((ts) => ts.filter((x) => x.step !== step));
          setLeftovers((l) => l.filter((n) => n !== step));
          for (const set of [setStored, setDesired]) {
            set((s) => {
              const { [step]: _, ...rest } = s;
              return rest;
            });
          }
          setFailedSaves((f) => {
            const { [step]: _, ...rest } = f;
            return rest;
          });
          setOpenStep((o) => (o === step ? null : o));
        } catch (e) {
          setOpError(t("review.delete_failed", { n: step, error: String(e) }));
          await resync();
        }
      });
    },
    [enqueue, outputDir, resync, t],
  );

  const reset = useCallback(() => {
    enqueue(async () => {
      try {
        await resetRedactions(outputDir);
        setStored({});
        setDesired({});
        setFailedSaves({});
        setOpError(null);
        setRefusalCleared(true);
        const loaded = await loadRedactions(outputDir);
        setLeftovers(loaded.leftover_steps ?? []);
        setUnreadable(loaded.unreadable ?? null);
      } catch (e) {
        setOpError(t("review.reset_failed", { error: String(e) }));
      }
    });
  }, [enqueue, outputDir, t]);

  const markedCount = thumbs.filter((thumb) => thumb.marked).length;
  const failedSteps = Object.keys(failedSaves).map(Number);
  const blocked =
    loading ||
    error !== null ||
    thumbs.length === 0 ||
    (markedCount > 0 && !reviewed) ||
    pending > 0 ||
    failedSteps.length > 0 ||
    leftovers.length > 0 ||
    unreadable !== null;
  const count = loading ? captureCount : thumbs.length;
  const open = thumbs.find((x) => x.step === openStep) ?? null;
  const rows = limbos(
    thumbs.map((x) => ({
      step: x.step,
      timestamp_seconds: times[x.step] ?? 0,
    })),
    visits,
  );
  const limboAfter = (after: number | null) =>
    rows
      .filter((r) => r.after === after)
      .map((r) => <LimboCard key={`limbo-${after}`} limbo={r} />);

  return (
    <div className="relative flex flex-col h-full bg-surface">
      {/* `contents` keeps the flex layout; `inert` keeps focus and clicks out
          of the screen behind the open step (#58). */}
      <div className="contents" inert={open !== null}>
        <div className="px-4 pt-4 pb-2">
          <p className="text-on-surface text-sm font-semibold">
            {t("review.title")}
          </p>
          <p
            className="text-on-surface-variant mt-1"
            style={{ fontSize: "0.75rem" }}
          >
            {/* Once thumbnails load they are the ground truth for the count; a
              folder picked from disk has no live capture telemetry at all. */}
            {elapsedSec > 0
              ? t("review.summary", {
                  count,
                  elapsed: formatElapsed(elapsedSec),
                })
              : t("review.summary_count_only", { count })}
            {deletedCount > 0 &&
              ` (${t("review.deleted_steps", { count: deletedCount })})`}
          </p>
          {!loading && thumbs.length === 0 && rows.length > 0 && (
            <p
              className="mt-2 text-xs"
              style={{ color: "rgba(255, 190, 80, 0.9)" }}
            >
              {t("review.no_steps_unrecorded", {
                apps: limboApps(t, rows[0].windows),
              })}
            </p>
          )}
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
                total: (loading ? captureCount : openedWith) + failedCaptures,
              })}
            </p>
          )}
          {[
            uploadError &&
              !refusalCleared &&
              t("review.upload_refused", { error: uploadError }),
            opError,
            ...failedSteps.map((n) =>
              t("review.redact_save_failed", { n, error: failedSaves[n] }),
            ),
          ]
            .filter(Boolean)
            .map((msg) => (
              <p
                key={String(msg)}
                role="alert"
                className="mt-2 text-xs"
                style={{ color: "#F87171" }}
              >
                {msg}
              </p>
            ))}
          {unreadable !== null && (
            <div
              role="alert"
              className="mt-2 flex flex-col gap-1.5 text-xs"
              style={{ color: "#F87171" }}
            >
              <p>{t("review.unreadable", { error: unreadable })}</p>
              {confirmReset ? (
                <div className="flex gap-2 items-center">
                  <span>{t("review.reset_confirm")}</span>
                  <button
                    className="btn-secondary px-2 py-1 text-xs"
                    onClick={() => {
                      setConfirmReset(false);
                      reset();
                    }}
                  >
                    {t("review.delete_confirm_yes")}
                  </button>
                  <button
                    className="btn-secondary px-2 py-1 text-xs"
                    onClick={() => setConfirmReset(false)}
                  >
                    {t("review.delete_confirm_no")}
                  </button>
                </div>
              ) : (
                <button
                  className="btn-secondary self-start px-2 py-1 text-xs"
                  onClick={() => setConfirmReset(true)}
                >
                  {t("review.reset")}
                </button>
              )}
            </div>
          )}
          {leftovers.map((n) => (
            <div
              key={n}
              role="alert"
              className="mt-2 flex gap-2 items-center text-xs"
              style={{ color: "#F87171" }}
            >
              <span>{t("review.leftover", { n })}</span>
              <button
                className="btn-secondary px-2 py-1 text-xs"
                onClick={() => removeStep(n)}
              >
                {t("review.leftover_retry")}
              </button>
            </div>
          ))}
        </div>

        <PipelineSelector />

        {/* Thumbnail strip */}
        <div className="flex-1 min-h-0 px-4 overflow-y-auto">
          {loading && (
            <p className="text-on-surface-variant text-xs">
              {t("review.loading")}
            </p>
          )}
          {error && (
            <p
              className="text-on-surface-variant text-xs"
              style={{ color: "#F87171" }}
            >
              {error}
            </p>
          )}
          {!loading && !error && (
            <>
              <p
                className="text-on-surface-variant mb-1.5"
                style={{ fontSize: "0.6875rem" }}
              >
                {t("review.open_hint")}
              </p>
              <div
                className="flex gap-2 overflow-x-auto pb-2"
                style={{ scrollbarWidth: "thin" }}
              >
                {thumbs.length > 0 && limboAfter(null)}
                {thumbs.flatMap((thumb) => [
                  <ReviewThumb
                    key={thumb.path}
                    thumb={thumb}
                    natural={natural[thumb.step] ?? { w: 0, h: 0 }}
                    stored={stored[thumb.step] ?? []}
                    desired={desired[thumb.step] ?? []}
                    saveFailed={Boolean(failedSaves[thumb.step])}
                    onOpen={() => {
                      setOpenStep(thumb.step);
                      setConfirmDelete(false);
                    }}
                    onNatural={(size) =>
                      setNatural((n) => ({ ...n, [thumb.step]: size }))
                    }
                  />,
                  ...limboAfter(thumb.step),
                ])}
              </div>
            </>
          )}
        </div>

        {markedCount > 0 && (
          <div className="px-4 pt-2 flex flex-col gap-1.5">
            <p
              className="text-on-surface-variant leading-snug"
              style={{ fontSize: "0.6875rem" }}
            >
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

      {open && (
        <StepEditor
          thumb={open}
          stored={stored[open.step] ?? []}
          desired={desired[open.step] ?? []}
          drawingDisabled={unreadable !== null}
          saving={pending > 0}
          saveError={failedSaves[open.step]}
          confirmDelete={confirmDelete}
          onAdd={(b) =>
            // A second identical box would share its React key and its removal.
            setStepBoxes(open.step, (list) =>
              hasBox(list, b) ? list : [...list, b],
            )
          }
          onRemove={(b) =>
            setStepBoxes(open.step, (list) =>
              list.filter((x) => boxKey(x) !== boxKey(b)),
            )
          }
          onRetrySave={() =>
            saveStep(open.step, desiredRef.current[open.step] ?? [])
          }
          onAskDelete={() => setConfirmDelete(true)}
          onCancelDelete={() => setConfirmDelete(false)}
          onDelete={() => {
            setConfirmDelete(false);
            removeStep(open.step);
          }}
          onClose={() => setOpenStep(null)}
        />
      )}
    </div>
  );
}
