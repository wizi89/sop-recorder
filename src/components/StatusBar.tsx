import { formatElapsed } from "../hooks/useElapsedTime";
import { useTranslation } from "../hooks/useTranslation";

interface StatusBarProps {
  message: string;
  busy: boolean;
  isError?: boolean;
  /**
   * Seconds since the current processing run began. Rendered while busy, so a
   * long wait is visibly a long wait rather than an unchanging screen.
   */
  elapsedSec?: number;
  /** No status message has arrived for a while; say so rather than say nothing. */
  stalled?: boolean;
}

export function StatusBar({ message, busy, isError, elapsedSec = 0, stalled }: StatusBarProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center gap-3">
      <p
        className={`text-center px-6 leading-relaxed ${isError ? "text-error" : "text-on-surface"}`}
        style={{ fontSize: "0.875rem" }}
      >
        {message}
      </p>
      {busy && (
        <div className="w-56 h-0.5 bg-surface-container-high rounded-full overflow-hidden">
          <div className="h-full bg-primary rounded-full animate-[indeterminate_1.5s_ease-in-out_infinite]" />
        </div>
      )}
      {busy && elapsedSec > 0 && (
        <p
          className="text-on-surface-variant text-center"
          style={{ fontSize: "0.6875rem" }}
        >
          {t("status.elapsed", { elapsed: formatElapsed(elapsedSec) })}
        </p>
      )}
      {busy && stalled && (
        <p
          className="text-on-surface-variant text-center px-6 leading-snug"
          style={{ fontSize: "0.6875rem", opacity: 0.85 }}
        >
          {t("status.still_waiting")}
        </p>
      )}
    </div>
  );
}
