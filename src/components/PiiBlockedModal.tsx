import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../hooks/useTranslation";
import type { TranslationKey } from "../i18n/de";

interface PiiFinding {
  source: string;
  entities: string[];
}

interface PiiBlockedModalProps {
  findings: PiiFinding[] | null;
  onDismiss: () => void;
}

export function PiiBlockedModal({ findings, onDismiss }: PiiBlockedModalProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const formatSource = (source: string): string => {
    if (source === "transcript") return t("pii.source_transcript");
    const match = source.match(/^step_(\d+)$/);
    if (match) return t("pii.source_step", { step: parseInt(match[1], 10) });
    return source;
  };

  const formatEntity = (entity: string): string => {
    const key = `pii.entity_${entity}` as TranslationKey;
    const result = t(key);
    return result === key ? entity : result;
  };

  const parsedFindings: PiiFinding[] | null = (() => {
    if (!findings) return null;
    if (!Array.isArray(findings)) return null;
    const sorted = [...findings as PiiFinding[]].sort((a, b) => {
      const numA = a.source.match(/^step_(\d+)$/)?.[1];
      const numB = b.source.match(/^step_(\d+)$/)?.[1];
      if (numA && numB) return parseInt(numA, 10) - parseInt(numB, 10);
      if (numA) return -1;
      if (numB) return 1;
      return 0;
    });
    return sorted;
  })();

  const handleCopy = () => {
    if (!parsedFindings) return;
    const text = parsedFindings
      .map((f) => `${formatSource(f.source)}: ${f.entities.map(formatEntity).join(", ")}`)
      .join("\n");
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-3">
      <div className="bg-surface rounded-lg w-full max-w-xs p-4 flex flex-col gap-2.5">
        <p className="text-on-surface text-sm font-semibold">{t("pii.title")}</p>
        <p className="text-on-surface-variant text-xs leading-snug">{t("pii.message")}</p>

        {/* Findings detail */}
        {parsedFindings && parsedFindings.length > 0 && (
          <div className="relative">
            <div className="bg-surface-container rounded p-2 flex flex-col gap-1 max-h-24 overflow-y-auto">
              {parsedFindings.map((f, i) => (
                <div key={i} className="text-xs">
                  <span className="text-on-surface font-medium">{formatSource(f.source)}:</span>{" "}
                  <span className="text-on-surface-variant">
                    {f.entities.map(formatEntity).join(", ")}
                  </span>
                </div>
              ))}
            </div>
            <button
              onClick={handleCopy}
              className="absolute top-1.5 right-1.5 bg-surface-bright hover:bg-surface-container-highest rounded px-2 py-0.5 border border-white/15 cursor-pointer text-xs"
            >
              <span className="text-on-surface">
                {copied ? t("pii.copied") : t("pii.copy")}
              </span>
            </button>
          </div>
        )}

        <p className="text-on-surface-variant text-xs leading-snug">{t("pii.settings_hint")}</p>

        <button onClick={onDismiss} className="btn-primary w-full py-1.5 text-xs mt-1">
          {t("pii.dismiss")}
        </button>

        {/* Legal disclaimer */}
        <div className="border-t border-white/10 pt-2">
          <p className="text-on-surface-variant leading-snug opacity-50 mb-1" style={{ fontSize: "0.6rem" }}>
            {t("pii.disclaimer")}
          </p>
          <div className="flex gap-3" style={{ fontSize: "0.6rem" }}>
            <button onClick={() => openUrl("https://flow.wizimate.com/legal")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
              {t("pii.link_legal")}
            </button>
            <button onClick={() => openUrl("https://flow.wizimate.com/privacy")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
              {t("pii.link_privacy")}
            </button>
            <button onClick={() => openUrl("https://flow.wizimate.com/terms")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
              {t("pii.link_terms")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
