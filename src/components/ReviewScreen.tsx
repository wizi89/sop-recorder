import { useEffect, useState } from "react";
import { useTranslation } from "../hooks/useTranslation";
import { listSessionScreenshots, readScreenshotBytes } from "../lib/tauri";
import { formatElapsed } from "../hooks/useElapsedTime";

interface ReviewScreenProps {
  outputDir: string;
  /** Count captured during the recording (from the live telemetry hook). */
  captureCount: number;
  /** Elapsed recording duration in whole seconds at the moment Stop was clicked. */
  elapsedSec: number;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Post-recording, pre-generation review screen.
 *
 * Shows a summary (count + elapsed) and a horizontal thumbnail strip of
 * captured screenshots. The user confirms to invoke generation or cancels
 * to discard and return to idle. When the session has zero screenshots the
 * parent should not render this screen at all -- the hook skips review and
 * transitions to idle with `no_clicks` status instead.
 */
export function ReviewScreen({
  outputDir,
  captureCount,
  elapsedSec,
  onConfirm,
  onCancel,
}: ReviewScreenProps) {
  const { t } = useTranslation();
  const [thumbs, setThumbs] = useState<Array<{ path: string; url: string }>>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const createdUrls: string[] = [];

    (async () => {
      try {
        const paths = await listSessionScreenshots(outputDir);
        // Read each file's bytes and wrap in a Blob URL for display.
        const entries = await Promise.all(
          paths.map(async (p) => {
            const bytes = await readScreenshotBytes(p);
            const blob = new Blob([bytes], { type: "image/png" });
            const url = URL.createObjectURL(blob);
            createdUrls.push(url);
            return { path: p, url };
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

  return (
    <div className="flex flex-col h-full bg-surface">
      <div className="px-4 pt-4 pb-2">
        <p className="text-on-surface text-sm font-semibold">
          {t("review.title")}
        </p>
        <p className="text-on-surface-variant mt-1" style={{ fontSize: "0.75rem" }}>
          {t("review.summary", {
            count: captureCount,
            elapsed: formatElapsed(elapsedSec),
          })}
        </p>
      </div>

      {/* Thumbnail strip */}
      <div className="flex-1 min-h-0 px-4 overflow-y-auto">
        {loading && (
          <p className="text-on-surface-variant text-xs">{t("review.loading")}</p>
        )}
        {error && (
          <p className="text-on-surface-variant text-xs" style={{ color: "#ff8080" }}>
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
              </div>
            ))}
          </div>
        )}
      </div>

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
          disabled={loading || thumbs.length === 0}
          style={{
            opacity: loading || thumbs.length === 0 ? 0.4 : 1,
            cursor: loading || thumbs.length === 0 ? "not-allowed" : "pointer",
          }}
        >
          {t("review.confirm")}
        </button>
      </div>
    </div>
  );
}
