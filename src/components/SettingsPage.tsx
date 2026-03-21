import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "../hooks/useTranslation";
import { getSettings, saveSettings, type AppSettings } from "../lib/tauri";

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
  });

  useEffect(() => {
    getSettings()
      .then(setSettings)
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
    </div>
  );
}
