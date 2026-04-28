import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../hooks/useTranslation";
import { getSettings, saveSettings, getQuota, type AppSettings } from "../lib/tauri";

interface SettingsPageProps {
  isDev: boolean;
}

export function SettingsPage({ isDev }: SettingsPageProps) {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<AppSettings>({
    output_dir: "",
    logs_dir: "",
    hide_from_screenshots: true,
    api_key: null,
    upload_target: null,
    skip_pii_check: false,
    pipeline_version: 1,
    generation_model: "azure/gpt-4.1",
  });
  const [showPiiConfirm, setShowPiiConfirm] = useState(false);
  const [advancedSettings, setAdvancedSettings] = useState(false);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch(() => {});
    getQuota()
      .then((q) => setAdvancedSettings(q.features?.advanced_settings ?? false))
      .catch(() => {});
  }, []);

  const handleSave = async () => {
    try {
      await saveSettings(settings);
      const win = getCurrentWindow();
      await win.close();
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  };

  const handleCancel = async () => {
    const win = getCurrentWindow();
    await win.close();
  };

  const handleBrowse = async (field: "output_dir" | "logs_dir") => {
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
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  pipeline_version: Number(e.target.value),
                }))
              }
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
            >
              <option value={1}>V1</option>
              <option value={2}>V2</option>
            </select>
          </div>
        )}

        {/* Generation model (advanced orgs only) */}
        {advancedSettings && (
          <div className="flex items-center justify-between">
            <label className="label-sm">{t("settings.model_label")}</label>
            <select
              value={settings.generation_model}
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  generation_model: e.target.value,
                }))
              }
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
            >
              <option value="azure/gpt-4.1">GPT-4.1</option>
              <option value="anthropic/claude-sonnet-4-6">Claude Sonnet 4.6</option>
            </select>
          </div>
        )}

        {/* Workflows directory */}
        <div>
          <label className="label-sm block mb-2">{t("settings.workflows_dir")}</label>
          <div className="flex gap-2">
            <input
              type="text"
              value={settings.output_dir}
              onChange={(e) =>
                setSettings((s) => ({ ...s, output_dir: e.target.value }))
              }
              className="input-field flex-1 rounded-lg px-3.5 py-2.5 text-sm"
            />
            <button
              onClick={() => handleBrowse("output_dir")}
              className="btn-primary px-4 py-2.5"
              style={{ fontSize: "0.6875rem" }}
            >
              {t("settings.choose")}
            </button>
          </div>
        </div>

        {/* Logs directory */}
        <div>
          <label className="label-sm block mb-2">{t("settings.logs_dir")}</label>
          <div className="flex gap-2">
            <input
              type="text"
              value={settings.logs_dir}
              onChange={(e) =>
                setSettings((s) => ({ ...s, logs_dir: e.target.value }))
              }
              className="input-field flex-1 rounded-lg px-3.5 py-2.5 text-sm"
            />
            <button
              onClick={() => handleBrowse("logs_dir")}
              className="btn-primary px-4 py-2.5"
              style={{ fontSize: "0.6875rem" }}
            >
              {t("settings.choose")}
            </button>
          </div>
        </div>

        {/* Dev mode: upload target */}
        {isDev && (
          <div className="flex items-center justify-between">
            <label className="label-sm">{t("settings.upload_to")}</label>
            <select
              value={settings.upload_target === "Local" ? "Local" : "Production"}
              onChange={(e) =>
                setSettings((s) => ({
                  ...s,
                  upload_target: e.target.value === "Local" ? "Local" : null,
                }))
              }
              className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
            >
              <option value="Local">Local</option>
              <option value="Production">Production</option>
            </select>
          </div>
        )}
      </div>

      {/* Actions */}
      <div className="flex gap-3 pt-5 mt-auto">
        <button onClick={handleCancel} className="btn-secondary flex-1 py-2.5 text-sm">
          {t("settings.cancel")}
        </button>
        <button onClick={handleSave} className="btn-primary flex-1 py-2.5 text-sm">
          {t("settings.save")}
        </button>
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
              <button onClick={() => openUrl("https://app.cogniclone.ai/legal")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
                {t("pii.link_legal")}
              </button>
              <button onClick={() => openUrl("https://app.cogniclone.ai/privacy")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
                {t("pii.link_privacy")}
              </button>
              <button onClick={() => openUrl("https://app.cogniclone.ai/terms")} className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0">
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
