import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../hooks/useTranslation";

/** Confirmation before the PII check is switched off. */
export function PiiConfirmModal({
  onCancel,
  onAccept,
}: {
  onCancel: () => void;
  onAccept: () => void;
}) {
  const { t } = useTranslation();
  return (
      <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
        <div className="bg-surface rounded-lg w-full max-w-sm p-5 flex flex-col gap-3">
          <p className="text-on-surface text-sm font-semibold">
            {t("pii.confirm_title")}
          </p>
          <div className="rounded p-3 max-h-72 overflow-y-auto flex flex-col gap-3"
            style={{ background: "rgba(255, 180, 50, 0.06)", border: "1px solid rgba(255, 180, 50, 0.15)" }}
          >
            <p className="text-on-surface text-xs leading-relaxed font-medium">
              {t("pii.confirm_intro")}
            </p>
            <p className="text-on-surface text-xs leading-relaxed" style={{ opacity: 0.85 }}>
              {t("pii.confirm_explain")}
            </p>
            <ul className="flex flex-col gap-1.5 pl-1 text-xs leading-relaxed" style={{ color: "rgba(255, 190, 80, 0.9)" }}>
              <li className="flex gap-2">
                <span className="shrink-0">&#x2022;</span>
                <span>{t("pii.confirm_bullet_1")}</span>
              </li>
              <li className="flex gap-2">
                <span className="shrink-0">&#x2022;</span>
                <span>{t("pii.confirm_bullet_2")}</span>
              </li>
              <li className="flex gap-2">
                <span className="shrink-0">&#x2022;</span>
                <span>{t("pii.confirm_bullet_3")}</span>
              </li>
            </ul>
            <p className="text-on-surface text-xs leading-relaxed font-medium">
              {t("pii.confirm_responsibility")}
            </p>
            <p className="text-on-surface text-xs leading-relaxed" style={{ opacity: 0.7 }}>
              {t("pii.confirm_scope")}
            </p>
          </div>
          <div className="flex gap-3" style={{ fontSize: "0.6rem" }}>
            <button onClick={() => openUrl("https://cogniclone.ai/impressum/")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
              {t("pii.link_legal")}
            </button>
            <button onClick={() => openUrl("https://cogniclone.ai/datenschutzerklaerung/")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
              {t("pii.link_privacy")}
            </button>
            <button onClick={() => openUrl("https://cogniclone.ai/nutzungsbedingungen/")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
              {t("pii.link_terms")}
            </button>
          </div>
          <div className="flex gap-3 mt-1">
            <button
              onClick={onCancel}
              className="btn-secondary flex-1 py-2 text-xs"
            >
              {t("pii.confirm_cancel")}
            </button>
            <button
              onClick={onAccept}
              className="flex-1 py-2 text-xs rounded-lg font-medium border-none cursor-pointer"
              style={{ background: "rgba(255, 160, 40, 0.2)", color: "rgba(255, 190, 80, 0.95)" }}
            >
              {t("pii.confirm_accept")}
            </button>
          </div>
        </div>
      </div>
  );
}
