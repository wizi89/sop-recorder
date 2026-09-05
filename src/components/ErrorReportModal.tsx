import { useEffect, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../hooks/useTranslation";
import { errorReportPath, type ErrorReport, type SubmittedReport } from "../lib/tauri";

interface ErrorReportModalProps {
  /** The report being decided on. Null once it has been sent: consent
   *  deletes the file, so there is nothing left to show but the number. */
  report: ErrorReport | null;
  /** No session yet: the report waits on disk and goes out after sign-in. */
  loggedIn: boolean;
  /** Set once the submission answered, so the dialog can show the number. */
  sent?: SubmittedReport | null;
  onGrant: (comment: string | null, alwaysSend: boolean) => void | Promise<void>;
  onDecline: () => void;
  onClose: () => void;
}

/**
 * Consent for one report, taken in front of its content (design D1).
 *
 * The plain-language summary comes first because a JSON dump alone is not
 * informed consent for the people who will see this dialog: they are testers
 * in an office, not developers, and a wall of braces reads as "trust us". The
 * verbatim view under "Details anzeigen" is what makes the summary honest --
 * and what it shows is exactly what would be sent, because scrubbing happened
 * when the report was written, not on the way out.
 *
 * The default answer is not to send. Closing the dialog declines.
 */
export function ErrorReportModal({
  report,
  loggedIn,
  sent,
  onGrant,
  onDecline,
  onClose,
}: ErrorReportModalProps) {
  const { t } = useTranslation();
  const [showDetails, setShowDetails] = useState(false);
  const [comment, setComment] = useState("");
  const [alwaysSend, setAlwaysSend] = useState(false);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);

  // `current` is `pending[0]`, so answering one report can slide the next one
  // into this same mounted component. Without this reset the dialog kept the
  // previous answer's `busy` flag and both buttons stayed disabled behind a
  // full-screen backdrop -- the app looked frozen. The comment is cleared for
  // a second reason: it describes the failure the user was just asked about,
  // and carrying it onto a different report would send the wrong context.
  const reportId = report?.report_id ?? null;
  useEffect(() => {
    setBusy(false);
    setComment("");
    setShowDetails(false);
    setAlwaysSend(false);
  }, [reportId]);

  const handleCopy = () => {
    if (!sent) return;
    navigator.clipboard.writeText(sent.number).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const handleReveal = async () => {
    if (!report) return;
    try {
      await revealItemInDir(await errorReportPath(report.report_id));
    } catch (e) {
      console.warn("Bericht-Datei konnte nicht angezeigt werden:", e);
    }
  };

  // Checked before anything reads `report`: a granted report is deleted from
  // disk once it is sent, so by the time this state is reached there is no
  // report left and only the number is worth showing.
  if (sent) {
    return (
      <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-3">
        <div className="bg-surface rounded-lg w-full max-w-xs p-4 flex flex-col gap-2.5">
          <p className="text-on-surface text-sm font-semibold">
            {t("report.sent_title")}
          </p>
          <div className="relative">
            <div className="bg-surface-container rounded p-2">
              <span className="text-on-surface text-xs font-medium">
                {t("report.sent_number", { number: sent.number })}
              </span>
            </div>
            <button
              onClick={handleCopy}
              className="absolute top-1.5 right-1.5 bg-surface-bright hover:bg-surface-container-highest rounded px-2 py-0.5 border border-white/15 cursor-pointer text-xs"
            >
              <span className="text-on-surface">
                {copied ? t("report.copied") : t("report.copy")}
              </span>
            </button>
          </div>
          <p className="text-on-surface-variant text-xs leading-snug">
            {t("report.sent_hint")}
          </p>
          <button onClick={onClose} className="btn-primary w-full py-1.5 text-xs mt-1">
            {t("report.close")}
          </button>
        </div>
      </div>
    );
  }

  if (!report) return null;

  // A panic found at launch is a crash the user already noticed. Saying so is
  // what makes the dialog make sense; the failure they are being asked about
  // happened in a session that is over.
  const isCrash = report.kind === "panic";

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-3">
      {/* The window is 440px tall and this dialog is the tallest thing in the
          app. Header and footer are pinned and only the middle scrolls, so the
          two consent buttons are on screen whatever the content does -- a
          dialog whose "Ablehnen" is below the fold is not a consent dialog. */}
      <div className="bg-surface rounded-lg w-full max-w-sm max-h-full flex flex-col overflow-hidden">
        <div className="shrink-0 flex flex-col gap-1.5 p-4 pb-2.5">
          <p className="text-on-surface text-sm font-semibold">
            {isCrash ? t("report.title_crash") : t("report.title")}
          </p>
          <p className="text-on-surface-variant text-xs leading-snug">
            {isCrash ? t("report.intro_crash") : t("report.intro")}{" "}
            {t("report.consent")}
          </p>
        </div>

        {/* min-h-0 lets this shrink inside the flex column; without it the
            scroll never engages and the card grows past the window again. */}
        <div className="flex-1 min-h-0 overflow-y-auto px-4 pb-1 flex flex-col gap-2.5">

        <div className="bg-surface-container rounded p-2 flex flex-col gap-2">
          <div>
            <p className="text-on-surface text-xs font-medium">
              {t("report.contains_title")}
            </p>
            <ul className="flex flex-col gap-0.5 mt-1 text-xs text-on-surface-variant leading-snug">
              {[1, 2, 3, 4].map((n) => (
                <li key={n} className="flex gap-2">
                  <span className="shrink-0">&#x2022;</span>
                  <span>{t(`report.contains_${n}` as "report.contains_1")}</span>
                </li>
              ))}
            </ul>
          </div>
          <div>
            <p className="text-on-surface text-xs font-medium">
              {t("report.excludes_title")}
            </p>
            <ul className="flex flex-col gap-0.5 mt-1 text-xs text-on-surface-variant leading-snug">
              {[1, 2, 3, 4].map((n) => (
                <li key={n} className="flex gap-2">
                  <span className="shrink-0">&#x2022;</span>
                  <span>{t(`report.excludes_${n}` as "report.excludes_1")}</span>
                </li>
              ))}
            </ul>
          </div>

          <button
            onClick={() => setShowDetails((v) => !v)}
            className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0 text-left"
            style={{ fontSize: "0.6875rem" }}
          >
            {showDetails ? t("report.hide_details") : t("report.show_details")}
          </button>
          {showDetails && (
            <pre
              data-testid="error-report-details"
              className="bg-surface-bright rounded p-2 overflow-auto max-h-40 text-on-surface-variant"
              style={{ fontSize: "0.5625rem", whiteSpace: "pre-wrap", wordBreak: "break-all" }}
            >
              {JSON.stringify({ ...report, consent: undefined }, null, 2)}
            </pre>
          )}
        </div>

        <label className="flex flex-col gap-1">
          <span className="label-sm">{t("report.comment_label")}</span>
          <textarea
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            placeholder={t("report.comment_placeholder")}
            rows={2}
            className="input-field rounded-lg px-2.5 py-2 text-xs resize-none"
          />
        </label>

        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={alwaysSend}
            onChange={(e) => setAlwaysSend(e.target.checked)}
          />
          <span className="text-on-surface-variant" style={{ fontSize: "0.6875rem" }}>
            {t("report.always_send")}
          </span>
        </label>

        {!loggedIn && (
          <div className="flex flex-col gap-1">
            <p className="text-on-surface-variant text-xs leading-snug">
              {t("report.signed_out_hint")}
            </p>
            <button
              onClick={handleReveal}
              className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0 text-left"
              style={{ fontSize: "0.6875rem" }}
            >
              {t("report.reveal_file")}
            </button>
          </div>
        )}

        </div>

        <div className="shrink-0 flex gap-3 p-4 pt-3 border-t border-white/10">
          <button
            onClick={onDecline}
            disabled={busy}
            className="btn-secondary flex-1 py-2 text-xs"
          >
            {t("report.decline")}
          </button>
          <button
            onClick={async () => {
              setBusy(true);
              try {
                await onGrant(comment.trim() === "" ? null : comment.trim(), alwaysSend);
              } catch (e) {
                // Swallowed on purpose. An escaping rejection reaches the
                // global handler in main.tsx, which would file a report about
                // the failure to send a report. The consent is already
                // recorded on disk, so the send retries at the next sign-in
                // (D7) and the user has lost nothing by closing this.
                console.warn("Fehlerbericht konnte nicht gesendet werden:", e);
              } finally {
                // A failed send must return control to the user. Leaving
                // `busy` set is a dead dialog with no way out.
                setBusy(false);
              }
            }}
            disabled={busy}
            className="btn-primary flex-1 py-2 text-xs"
          >
            {/* One label in both states. The signed-out hint right above the
                buttons already says the report waits for the next sign-in, and
                repeating it here overflowed the button. */}
            {t("report.send")}
          </button>
        </div>
      </div>
    </div>
  );
}
