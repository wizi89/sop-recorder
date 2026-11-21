import { isReportable } from "../lib/serverErrors";
import { createErrorReport, debugTriggerFailure, errorReportPhase } from "../lib/tauri";

/**
 * Dev-only buttons that trigger each kind of failure. `silent` is true when
 * error reports are off ("never" or forced off by the organisation).
 */
export function DevFailureTriggers({
  silent,
  forcedOff,
}: {
  silent: boolean;
  forcedOff: boolean;
}) {
  return (
    <div className="flex flex-col gap-2">
      <label className="label-sm">Fehler auslösen (nur Entwicklung)</label>
      {silent && (
        // Silence is the correct behaviour in this mode, but a button
        // that does nothing and says nothing is indistinguishable from a
        // broken one -- which cost a debugging round.
        <p
          className="text-on-surface-variant leading-snug"
          style={{ fontSize: "0.625rem" }}
        >
          {forcedOff
            ? "Fehlerberichte sind von der Organisation deaktiviert: Diese Schaltflächen erzeugen keinen Bericht."
            : "Fehlerberichte stehen auf \u201eNie\u201c: Diese Schaltflächen erzeugen absichtlich keinen Bericht."}
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        {[
          ["command_error", "Command-Fehler"],
          ["expected_command_error", "Command-Fehler (erwartet)"],
          ["background_panic", "Panic (Hintergrund)"],
          ["main_thread_panic", "Panic (Haupt-Thread)"],
          ["ui_error", "UI-Fehler"],
        ].map(([kind, label]) => (
          <button
            key={kind}
            onClick={() => {
              if (kind === "ui_error") {
                // Thrown async so it reaches the window `error` handler
                // rather than being caught by React's boundary.
                setTimeout(() => {
                  throw new Error("Absichtlicher UI-Testfehler");
                });
                return;
              }
              void debugTriggerFailure(kind).catch((e) => {
                // A rejected command is only half the path. The other
                // half is the classifier (D6): a failure the UI can
                // explain is not reported, and only what is left becomes
                // a report. Doing both here is what makes this button
                // exercise the real flow rather than just log.
                const message = String(e);
                if (!isReportable(message)) {
                  console.warn("Klassifiziert als erwartetes Ergebnis, kein Bericht:", message);
                  return;
                }
                void createErrorReport("command_error", errorReportPhase(), message).catch(
                  (err) => console.warn("Bericht konnte nicht erstellt werden:", err),
                );
              });
            }}
            className="btn-secondary px-3 py-1.5"
            style={{ fontSize: "0.6875rem" }}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
