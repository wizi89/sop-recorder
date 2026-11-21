import { useTranslation } from "../hooks/useTranslation";
import type { useUpdater } from "../hooks/useUpdater";

/** The bar offering an available update; hidden while recording or once dismissed. */
export function UpdateBanner({
  updater,
  recording,
}: {
  updater: ReturnType<typeof useUpdater>;
  recording: boolean;
}) {
  const { t } = useTranslation();
  const showBanner =
    !updater.dismissed &&
    !recording &&
    (updater.status === "available" || updater.status === "downloading");
  if (!showBanner) return null;
  return (
    <div
      className="w-full flex items-center"
      style={{
        background: "linear-gradient(135deg, #1E8A93, #2CB5C0)",
        flexShrink: 0,
      }}
    >
      <button
        onClick={updater.install}
        disabled={updater.status === "downloading"}
        className="flex-1 py-2 text-xs font-medium text-center"
        style={{
          background: "transparent",
          color: "#fff",
          border: "none",
          cursor: updater.status === "downloading" ? "wait" : "pointer",
        }}
      >
        {updater.status === "downloading"
          ? t("update.downloading")
          : `${t("update.available", { version: updater.version ?? "" })} — ${t("update.install")}`}
      </button>
      {updater.status !== "downloading" && (
        <button
          onClick={updater.dismiss}
          className="px-2 py-2 text-xs"
          style={{
            background: "transparent",
            color: "rgba(255,255,255,0.7)",
            border: "none",
            cursor: "pointer",
            lineHeight: 1,
          }}
        >
          ✕
        </button>
      )}
    </div>
  );
}
