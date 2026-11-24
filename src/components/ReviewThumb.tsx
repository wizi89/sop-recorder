import { useTranslation } from "../hooks/useTranslation";
import type { RedactionBox } from "../lib/tauri";
import { containRect } from "../lib/redactionGeometry";
import { BoxOverlay, type Thumb } from "./StepEditor";

/** One step in the review screen's strip; a click opens it. */
export function ReviewThumb({
  thumb,
  natural,
  stored,
  desired,
  saveFailed,
  onOpen,
  onNatural,
}: {
  thumb: Thumb;
  natural: { w: number; h: number };
  stored: RedactionBox[];
  desired: RedactionBox[];
  saveFailed: boolean;
  onOpen: () => void;
  onNatural: (size: { w: number; h: number }) => void;
}) {
  const { t } = useTranslation();
  const boxCount = stored.length;
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex-shrink-0 rounded border text-left"
      style={{
        width: 96,
        borderColor: saveFailed ? "#F87171" : "rgba(255,255,255,0.12)",
        background: "rgba(255,255,255,0.04)",
      }}
    >
      <div style={{ position: "relative", width: 96, height: 54 }}>
        <img
          src={thumb.url}
          alt={t("review.step_label", { n: thumb.step })}
          onLoad={(e) => {
            const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
            onNatural({ w, h });
          }}
          style={{
            width: "100%",
            height: 54,
            objectFit: "contain",
            borderRadius: "3px 3px 0 0",
          }}
        />
        <BoxOverlay
          content={containRect(96, 54, natural.w, natural.h)}
          natural={natural}
          stored={stored}
          desired={desired}
        />
      </div>
      <div
        className="text-on-surface-variant text-center"
        style={{ fontSize: "0.625rem", padding: "2px 0" }}
      >
        {t("review.step_label", { n: thumb.step })}
      </div>
      {boxCount > 0 && (
        <div
          className="text-on-surface-variant text-center"
          style={{ fontSize: "0.625rem" }}
        >
          {t("review.redact_boxes", { count: boxCount })}
        </div>
      )}
      {thumb.marked && (
        <div
          className="text-center"
          style={{
            fontSize: "0.625rem",
            color: "rgba(255, 190, 80, 0.9)",
          }}
        >
          {t("review.marked_badge")}
        </div>
      )}
    </button>
  );
}
