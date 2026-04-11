import { useTranslation } from "../hooks/useTranslation";

interface RateLimitModalProps {
  count: number | null;
  limit: number | null;
  onDismiss: () => void;
  onUpgrade?: () => void;
}

export function RateLimitModal({ count, limit, onDismiss, onUpgrade }: RateLimitModalProps) {
  const { t } = useTranslation();

  const hasNumbers = count !== null && limit !== null;
  const message = hasNumbers
    ? t("quota.exceeded_message", { limit: limit as number })
    : t("quota.exceeded_message_generic");

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-3">
      <div className="bg-surface rounded-lg w-full max-w-xs p-4 flex flex-col gap-2.5">
        <p className="text-on-surface text-sm font-semibold">{t("quota.exceeded_title")}</p>
        <p className="text-on-surface-variant text-xs leading-snug">{message}</p>

        {hasNumbers && (
          <div className="bg-surface-container rounded p-2">
            <span className="text-on-surface text-xs font-medium">
              {t("quota.used", { count: count as number, limit: limit as number })}
            </span>
          </div>
        )}

        <div className="flex flex-col gap-1.5 mt-1">
          {onUpgrade && (
            <button onClick={onUpgrade} className="btn-primary w-full py-1.5 text-xs">
              {t("quota.exceeded_upgrade")}
            </button>
          )}
          <button
            onClick={onDismiss}
            className={onUpgrade ? "btn-secondary w-full py-1.5 text-xs" : "btn-primary w-full py-1.5 text-xs"}
          >
            {t("quota.exceeded_dismiss")}
          </button>
        </div>
      </div>
    </div>
  );
}
