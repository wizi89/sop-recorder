import { useTranslation } from "../hooks/useTranslation";
import type { Limbo, UnrecordedWindow } from "../lib/limbos";

type T = ReturnType<typeof useTranslation>["t"];

/** App names only: a window title can hold private content. */
export function limboApps(t: T, windows: UnrecordedWindow[]): string {
  return windows
    .map((w) =>
      w.kind === "app"
        ? w.name
        : t(
            w.kind === "security_prompt"
              ? "review.limbo_security_prompt"
              : "review.limbo_unknown_app",
          ),
    )
    .join(", ");
}

/** A limbo in the review screen's strip. Informational: no tick, no block. */
export function LimboCard({ limbo }: { limbo: Limbo }) {
  const { t } = useTranslation();
  return (
    <div
      data-strip-item={`limbo-${limbo.after ?? 0}`}
      className="flex-shrink-0 rounded border px-1.5 py-1 leading-snug"
      style={{
        width: 96,
        fontSize: "0.625rem",
        borderStyle: "dashed",
        borderColor: "rgba(255, 180, 50, 0.3)",
        color: "rgba(255, 190, 80, 0.9)",
      }}
    >
      {t("review.limbo", { apps: limboApps(t, limbo.windows) })}
    </div>
  );
}
