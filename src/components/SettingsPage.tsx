import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../hooks/useTranslation";
import { isReportable } from "../lib/serverErrors";
import { emit } from "@tauri-apps/api/event";
import {
  getSettings,
  saveSettings,
  getQuota,
  logout,
  areErrorReportsForcedOff,
  debugTriggerFailure,
  setErrorReportPhase,
  createErrorReport,
  errorReportPhase,
  type AppSettings,
  type ErrorReportMode,
  type GenerationSettings,
} from "../lib/tauri";

interface SettingsPageProps {
  isDev: boolean;
}

const FALLBACK_GENERATION_SETTINGS: GenerationSettings = {
  pipeline_versions: [1, 2],
  models: ["azure/gpt-4.1"],
  default_model: "azure/gpt-4.1",
};

function modelLabel(model: string): string {
  const name = model.split("/").pop() || model;
  return name
    .replace(/^gpt-/, "GPT-")
    .replace(/^claude-/, "Claude ")
    .replace(/-/g, " ");
}

export function SettingsPage({ isDev }: SettingsPageProps) {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<AppSettings>({
    output_dir: "",
    logs_dir: "",
    hide_from_screenshots: true,
    upload_target: null,
    skip_pii_check: false,
    pipeline_version: 1,
    generation_model: "azure/gpt-4.1",
    error_reports: "ask",
  });
  // Until the stored settings arrive, the form shows defaults the user must not
  // be able to act on: the load used to replace the whole form state when it
  // resolved, so a toggle flipped before then was silently reverted and the
  // save wrote the old value. That is the "first save does not stick" report
  // from 2026-09-03 -- made reachable by `get_settings` waiting on the macOS
  // keychain, which after a reinstall can take seconds.
  const [loaded, setLoaded] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [showPiiConfirm, setShowPiiConfirm] = useState(false);
  // An installation can switch error reports off for everyone (design D1).
  // The control then shows the chosen mode but cannot be changed, and says
  // who decided -- a disabled control with no explanation reads as a bug.
  const [errorReportsForcedOff, setErrorReportsForcedOff] = useState(false);
  const [advancedSettings, setAdvancedSettings] = useState(false);
  const [generationSettings, setGenerationSettings] = useState<GenerationSettings>(
    FALLBACK_GENERATION_SETTINGS,
  );
  // Track the upload_target loaded from disk so we can detect a backend
  // switch on save. A switch invalidates the current session (tokens are
  // backend-bound), so the user has to log in again.
  const [initialUploadTarget, setInitialUploadTarget] = useState<string | null>(null);

  // This window is on top of whatever the main window shows, so while it is
  // open it is what the user is doing. The main window reclaims the phase when
  // it regains focus; nothing here has to undo it, which is just as well since
  // an unmount is not guaranteed when a window closes.
  useEffect(() => {
    void setErrorReportPhase("settings");
  }, []);

  useEffect(() => {
    getSettings()
      .then((s) => {
        setSettings(s);
        setInitialUploadTarget(s.upload_target);
      })
      .catch((e) => {
        console.error("Failed to load settings:", e);
      })
      // Enabled either way. A form that stays disabled after a failed load is
      // a window the user cannot close by saving, and the values on screen are
      // then the defaults -- which is worth letting them correct.
      .finally(() => setLoaded(true));
    areErrorReportsForcedOff().then(setErrorReportsForcedOff).catch(() => {});
    getQuota()
      .then((q) => {
        setAdvancedSettings(q.features?.advanced_settings ?? false);
        if (q.generation_settings?.models?.length) {
          setGenerationSettings(q.generation_settings);
          setSettings((s) => {
            if (q.generation_settings!.models.includes(s.generation_model)) {
              return s;
            }
            return {
              ...s,
              generation_model:
                q.generation_settings!.default_model || q.generation_settings!.models[0],
            };
          });
        }
      })
      .catch(() => {});
  }, []);

  const handleSave = async () => {
    setSaveError(null);
    try {
      await saveSettings(settings);
      // Backend switch invalidates the current session token. Log out
      // and tell the main window so it bounces to the login screen
      // instead of letting the next request 401 silently.
      if (settings.upload_target !== initialUploadTarget) {
        try {
          await logout();
        } catch (e) {
          console.warn("Logout after backend switch failed:", e);
        }
        await emit("auth:session_expired");
      }
      const win = getCurrentWindow();
      await win.close();
    } catch (e) {
      // The window stays open. Closing it on a failed save is what made a lost
      // write indistinguishable from a successful one.
      console.error("Failed to save settings:", e);
      setSaveError(String(e));
    }
  };

  const handleCancel = async () => {
    const win = getCurrentWindow();
    await win.close();
  };

  const handleBrowse = async (field: "output_dir") => {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: t("settings.choose"),
    });
    if (selected) {
      setSettings((s) => ({ ...s, [field]: selected }));
    }
  };

  const handlePiiToggle = () => {
    if (!settings.skip_pii_check) {
      // Turning PII check OFF -> show confirmation
      setShowPiiConfirm(true);
    } else {
      // Turning PII check back ON -> no confirmation needed
      setSettings((s) => ({ ...s, skip_pii_check: false }));
    }
  };

  const confirmDisablePii = () => {
    setSettings((s) => ({ ...s, skip_pii_check: true }));
    setShowPiiConfirm(false);
  };

  return (
    <div className="flex flex-col h-screen p-5 bg-surface">
      <div className="flex-1 flex flex-col gap-6 overflow-y-auto">
        {/* Hide from screenshots */}
        <div className="flex items-center justify-between">
          <label className="label-sm">{t("settings.hide_screenshots")}</label>
          <button
            className="switch-track"
            data-checked={settings.hide_from_screenshots}
            disabled={!loaded}
            onClick={() =>
              setSettings((s) => ({
                ...s,
                hide_from_screenshots: !s.hide_from_screenshots,
              }))
            }
          >
            <span
              className="switch-thumb"
              style={{ left: settings.hide_from_screenshots ? 21 : 3 }}
            />
          </button>
        </div>

        {/* Skip PII check */}
        <div className="flex items-center justify-between">
          <label className="label-sm">{t("settings.skip_pii_check")}</label>
          <button
            className="switch-track"
            data-checked={settings.skip_pii_check}
            disabled={!loaded}
            onClick={handlePiiToggle}
          >
            <span
              className="switch-thumb"
              style={{ left: settings.skip_pii_check ? 21 : 3 }}
            />
          </button>
        </div>

        {/* Pipeline version (advanced orgs only) */}
        {advancedSettings && (
          <div className="flex items-center justify-between">
            <label className="label-sm">{t("settings.pipeline_label")}</label>
            <select
              value={settings.pipeline_version}
              disabled={!loaded}
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  pipeline_version: Number(e.target.value),
                }))
              }
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
            >
              {(generationSettings.pipeline_versions.length
                ? generationSettings.pipeline_versions
                : FALLBACK_GENERATION_SETTINGS.pipeline_versions
              ).map((version) => (
                <option key={version} value={version}>
                  V{version}
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Generation model (advanced orgs only) */}
        {advancedSettings && (
          <div className="flex items-center justify-between">
            <label className="label-sm">{t("settings.model_label")}</label>
            <select
              value={settings.generation_model}
              disabled={!loaded}
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  generation_model: e.target.value,
                }))
              }
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
            >
              {(generationSettings.models.length
                ? generationSettings.models
                : FALLBACK_GENERATION_SETTINGS.models
              ).map((model) => (
                <option key={model} value={model}>
                  {modelLabel(model)}
                </option>
              ))}
            </select>
          </div>
        )}

        {/* Error reports (design D1). Three modes, and a note when the
            installation has taken the choice away. */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center justify-between">
            <label className="label-sm" htmlFor="error-reports-mode">
              {t("settings.error_reports")}
            </label>
            <select
              id="error-reports-mode"
              value={errorReportsForcedOff ? "never" : settings.error_reports}
              disabled={errorReportsForcedOff || !loaded}
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  error_reports: e.target.value as ErrorReportMode,
                }))
              }
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
              style={{
                opacity: errorReportsForcedOff ? 0.5 : 1,
                cursor: errorReportsForcedOff ? "not-allowed" : "pointer",
              }}
            >
              <option value="ask">{t("settings.error_reports_ask")}</option>
              <option value="always">{t("settings.error_reports_always")}</option>
              <option value="never">{t("settings.error_reports_never")}</option>
            </select>
          </div>
          <p
            className="text-on-surface-variant leading-snug"
            style={{ fontSize: "0.625rem" }}
          >
            {errorReportsForcedOff
              ? t("settings.error_reports_disabled_by_org")
              : t("settings.error_reports_hint")}
          </p>
        </div>

        {/* Dev-only failure triggers. `import.meta.env.DEV` keeps them out of
            the production bundle, and the Rust side is behind
            `debug_assertions` as well, so a release build has neither the
            button nor the panic. There is no other way to reach a panic by
            hand: `withGlobalTauri` is off, so the console cannot invoke. */}
        {import.meta.env.DEV && (
          <div className="flex flex-col gap-2">
            <label className="label-sm">Fehler auslösen (nur Entwicklung)</label>
            {(settings.error_reports === "never" || errorReportsForcedOff) && (
              // Silence is the correct behaviour in this mode, but a button
              // that does nothing and says nothing is indistinguishable from a
              // broken one -- which cost a debugging round.
              <p
                className="text-on-surface-variant leading-snug"
                style={{ fontSize: "0.625rem" }}
              >
                {errorReportsForcedOff
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
        )}

        {/* Workflows directory */}
        <div>
          <label className="label-sm block mb-2">{t("settings.workflows_dir")}</label>
          <div className="flex gap-2">
            <input
              type="text"
              value={settings.output_dir}
              disabled={!loaded}
              onChange={(e) =>
                setSettings((s) => ({ ...s, output_dir: e.target.value }))
              }
              className="input-field flex-1 rounded-lg px-3.5 py-2.5 text-sm"
            />
            <button
              onClick={() => handleBrowse("output_dir")}
              disabled={!loaded}
              className="btn-primary px-4 py-2.5"
              style={{ fontSize: "0.6875rem" }}
            >
              {t("settings.choose")}
            </button>
          </div>
        </div>

        {/* Logs directory. Shown, not chosen: the log plugin's target is fixed
            before the app handle exists, so a path typed here would be a
            setting that changes nothing -- which is what it was, and it named a
            directory that did not exist on macOS besides. */}
        <div>
          <label className="label-sm block mb-2" htmlFor="logs-dir">
            {t("settings.logs_dir")}
          </label>
          <div className="flex gap-2">
            <input
              id="logs-dir"
              type="text"
              value={settings.logs_dir}
              readOnly
              className="input-field flex-1 rounded-lg px-3.5 py-2.5 text-sm"
              style={{ opacity: 0.7 }}
            />
            <button
              onClick={() => revealItemInDir(settings.logs_dir)}
              disabled={!loaded || !settings.logs_dir}
              className="btn-primary px-4 py-2.5"
              style={{ fontSize: "0.6875rem" }}
            >
              {t("settings.reveal")}
            </button>
          </div>
        </div>

        {/* Upload target (advanced orgs only). Server-driven via
            features.advanced_settings so we don't ship Production/Staging
            switching to end users -- only the cogniclone org sees this.
            The Local option only renders in dev builds since it points
            at localhost; advanced orgs running a release binary should
            only ever see Production/Staging. */}
        {advancedSettings && (
          <div className="flex items-center justify-between">
            <label className="label-sm">{t("settings.upload_to")}</label>
            <select
              value={
                settings.upload_target === "Staging"
                  ? "Staging"
                  : settings.upload_target === "Local" && isDev
                    ? "Local"
                    : "Production"
              }
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  upload_target:
                    e.target.value === "Staging" || (e.target.value === "Local" && isDev)
                      ? e.target.value
                      : null,
                }))
              }
              disabled={!loaded}
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
            >
              {isDev && <option value="Local">Local</option>}
              <option value="Staging">Staging</option>
              <option value="Production">Production</option>
            </select>
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex flex-col gap-2 pt-5 mt-auto">
        {saveError && (
          <p
            role="alert"
            className="rounded px-2 py-1.5 leading-snug"
            style={{
              fontSize: "0.6875rem",
              background: "rgba(255, 100, 90, 0.08)",
              border: "1px solid rgba(255, 100, 90, 0.2)",
              color: "rgba(255, 140, 130, 0.95)",
            }}
          >
            {t("settings.save_failed", { error: saveError })}
          </p>
        )}
        <div className="flex gap-3">
          <button onClick={handleCancel} className="btn-secondary flex-1 py-2.5 text-sm">
            {t("settings.cancel")}
          </button>
          <button
            onClick={handleSave}
            disabled={!loaded}
            className="btn-primary flex-1 py-2.5 text-sm"
          >
            {loaded ? t("settings.save") : t("settings.loading")}
          </button>
        </div>
      </div>

      {/* PII disable confirmation modal */}
      {showPiiConfirm && (
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
                onClick={() => setShowPiiConfirm(false)}
                className="btn-secondary flex-1 py-2 text-xs"
              >
                {t("pii.confirm_cancel")}
              </button>
              <button
                onClick={confirmDisablePii}
                className="flex-1 py-2 text-xs rounded-lg font-medium border-none cursor-pointer"
                style={{ background: "rgba(255, 160, 40, 0.2)", color: "rgba(255, 190, 80, 0.95)" }}
              >
                {t("pii.confirm_accept")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
