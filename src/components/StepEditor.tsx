import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { useTranslation } from "../hooks/useTranslation";
import type { RedactionBox } from "../lib/tauri";
import {
  containRect,
  isClick,
  toDisplayRect,
  toImageBox,
  type ContentRect,
} from "../lib/redactionGeometry";

export interface Thumb {
  /** The number in the file name; labels and every command use it (D4). */
  step: number;
  path: string;
  url: string;
  marked: boolean;
}

export const boxKey = (b: RedactionBox) => b.join(",");
export const hasBox = (list: RedactionBox[] | undefined, b: RedactionBox) =>
  (list ?? []).some((x) => boxKey(x) === boxKey(b));

/**
 * Boxes drawn over an image shown with `object-fit: contain`, mapped through
 * the image's rendered content rectangle (D4). Stored boxes are opaque; boxes
 * whose save has not returned (or failed) are outlines.
 */
export function BoxOverlay({
  content,
  natural,
  stored,
  desired,
  onRemove,
}: {
  content: ContentRect;
  natural: { w: number; h: number };
  stored: RedactionBox[];
  desired: RedactionBox[];
  onRemove?: (b: RedactionBox) => void;
}) {
  const { t } = useTranslation();
  if (natural.w <= 0 || natural.h <= 0) return null;
  // Stored but no longer desired: its removal is still being saved, so it is
  // still what would be uploaded and still shown opaque.
  const shown = [...desired, ...stored.filter((b) => !hasBox(desired, b))];
  return (
    <>
      {shown.map((b) => {
        const r = toDisplayRect(b, content, natural.w, natural.h);
        const applied = hasBox(stored, b);
        return (
          <div
            key={boxKey(b)}
            data-testid={applied ? "redaction-applied" : "redaction-pending"}
            style={{
              position: "absolute",
              left: r.left,
              top: r.top,
              width: r.width,
              height: r.height,
              background: applied ? "#000" : "transparent",
              border: applied ? "none" : "1px dashed #F87171",
              boxSizing: "border-box",
              pointerEvents: "none",
            }}
          >
            {onRemove && hasBox(desired, b) && (
              <button
                type="button"
                aria-label={t("review.redact_remove")}
                onPointerDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation();
                  onRemove(b);
                }}
                style={{
                  position: "absolute",
                  right: 0,
                  top: 0,
                  pointerEvents: "auto",
                  // A 24x24 px hit area at the least (WCAG 2.5.8).
                  minWidth: 24,
                  minHeight: 24,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  fontSize: "0.75rem",
                  lineHeight: 1,
                  padding: 0,
                  background: "rgba(255,255,255,0.85)",
                  color: "#000",
                }}
              >
                ×
              </button>
            )}
          </div>
        );
      })}
    </>
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
 * A step opens enlarged on click; there the user draws opaque boxes and
 * deletes the step (review-redaction). Boxes are stored as they are drawn and
 * burned into the uploaded bytes only; the files on disk stay the originals.
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

/**
 * One step enlarged, with `object-fit: contain`. Dragging draws a box; boxes
 * are mapped through the rendered content rectangle so they are stored in
 * saved-image pixels whatever the window size (D4).
 */
export function StepEditor({
  thumb,
  stored,
  desired,
  drawingDisabled,
  saving,
  saveError,
  confirmDelete,
  onAdd,
  onRemove,
  onRetrySave,
  onAskDelete,
  onCancelDelete,
  onDelete,
  onClose,
}: {
  thumb: Thumb;
  stored: RedactionBox[];
  desired: RedactionBox[];
  drawingDisabled: boolean;
  saving: boolean;
  /** Why the last save of this step failed; unset when it did not. */
  saveError?: string;
  confirmDelete: boolean;
  onAdd: (b: RedactionBox) => void;
  onRemove: (b: RedactionBox) => void;
  onRetrySave: () => void;
  onAskDelete: () => void;
  onCancelDelete: () => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const frame = useRef<HTMLDivElement>(null);
  const img = useRef<HTMLImageElement>(null);
  const done = useRef<HTMLButtonElement>(null);
  // Focus starts inside the dialog, so Escape and Tab work from the first key.
  useEffect(() => done.current?.focus(), []);
  const [layout, setLayout] = useState<{ content: ContentRect; w: number; h: number } | null>(
    null,
  );
  const [drag, setDrag] = useState<{ a: { x: number; y: number }; b: { x: number; y: number } } | null>(
    null,
  );

  const measure = useCallback(() => {
    const el = frame.current;
    const im = img.current;
    if (!el || !im) return;
    const r = el.getBoundingClientRect();
    const w = im.naturalWidth;
    const h = im.naturalHeight;
    setLayout({ content: containRect(r.width, r.height, w, h), w, h });
  }, []);

  // The frame changes size with the window and when the delete confirmation
  // opens above it; the boxes follow it.
  useEffect(() => {
    const el = frame.current;
    if (el && typeof ResizeObserver !== "undefined") {
      const ro = new ResizeObserver(measure);
      ro.observe(el);
      return () => ro.disconnect();
    }
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [measure]);

  const point = (e: ReactPointerEvent) => {
    const r = frame.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  const finish = (e: ReactPointerEvent) => {
    if (!drag) return;
    const end = point(e);
    setDrag(null);
    if (isClick(drag.a, end)) return;
    // Measured at the moment of drawing, so a resize since load cannot skew it.
    const el = frame.current!;
    const im = img.current!;
    const r = el.getBoundingClientRect();
    const content = containRect(r.width, r.height, im.naturalWidth, im.naturalHeight);
    const box = toImageBox(drag.a, end, content, im.naturalWidth, im.naturalHeight);
    if (box) onAdd(box);
  };

  const draft =
    drag &&
    ({
      left: Math.min(drag.a.x, drag.b.x),
      top: Math.min(drag.a.y, drag.b.y),
      width: Math.abs(drag.a.x - drag.b.x),
      height: Math.abs(drag.a.y - drag.b.y),
    } as const);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t("review.step_label", { n: thumb.step })}
      className="absolute inset-0 z-10 flex flex-col bg-surface"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="flex items-center gap-2 px-4 pt-3 pb-2">
        <p className="text-on-surface text-sm font-semibold flex-1">
          {t("review.step_label", { n: thumb.step })}
          {saving && (
            <span className="text-on-surface-variant ml-2 font-normal text-xs">
              {t("review.redact_saving")}
            </span>
          )}
        </p>
        {saveError !== undefined && (
          <button className="btn-secondary px-2 py-1 text-xs" onClick={onRetrySave}>
            {t("review.redact_retry")}
          </button>
        )}
        {/* With the box file unreadable a delete cannot finish its last part,
            so only the reset is offered (D5). */}
        <button
          className="btn-secondary px-2 py-1 text-xs"
          onClick={onAskDelete}
          disabled={drawingDisabled}
        >
          {t("review.delete_step")}
        </button>
        <button ref={done} className="btn-primary px-2 py-1 text-xs" onClick={onClose}>
          {t("review.close")}
        </button>
      </div>
      {/* The alert list behind the dialog is covered, so the error is said here. */}
      {saveError !== undefined && (
        <p role="alert" className="px-4 pb-2 text-xs" style={{ color: "#F87171" }}>
          {t("review.redact_save_failed", { n: thumb.step, error: saveError })}
        </p>
      )}
      {confirmDelete && (
        <div className="px-4 pb-2 flex flex-col gap-1.5 text-xs">
          <p className="text-on-surface-variant leading-snug">
            {t("review.delete_confirm", { n: thumb.step })}
          </p>
          <div className="flex gap-2">
            <button className="btn-secondary px-2 py-1 text-xs" onClick={onDelete}>
              {t("review.delete_confirm_yes")}
            </button>
            <button className="btn-secondary px-2 py-1 text-xs" onClick={onCancelDelete}>
              {t("review.delete_confirm_no")}
            </button>
          </div>
        </div>
      )}
      <p className="text-on-surface-variant px-4 pb-2" style={{ fontSize: "0.6875rem" }}>
        {t("review.redact_hint")}
      </p>
      <div className="flex-1 min-h-0 px-4 pb-3">
        <div
          ref={frame}
          data-testid="redaction-frame"
          className="relative w-full h-full select-none"
          style={{ cursor: drawingDisabled ? "default" : "crosshair", touchAction: "none" }}
          onPointerDown={(e) => {
            if (drawingDisabled || e.button !== 0) return;
            e.currentTarget.setPointerCapture?.(e.pointerId);
            const p = point(e);
            setDrag({ a: p, b: p });
          }}
          onPointerMove={(e) => drag && setDrag({ a: drag.a, b: point(e) })}
          onPointerUp={finish}
          onPointerCancel={() => setDrag(null)}
        >
          <img
            ref={img}
            src={thumb.url}
            alt={t("review.step_enlarged", { n: thumb.step })}
            draggable={false}
            onLoad={measure}
            style={{ width: "100%", height: "100%", objectFit: "contain" }}
          />
          {layout && (
            <BoxOverlay
              content={layout.content}
              natural={{ w: layout.w, h: layout.h }}
              stored={stored}
              desired={desired}
              onRemove={onRemove}
            />
          )}
          {draft && (
            <div
              style={{
                position: "absolute",
                ...draft,
                border: "1px dashed #fff",
                pointerEvents: "none",
              }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
