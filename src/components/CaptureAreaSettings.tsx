import type { Dispatch, SetStateAction } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "../hooks/useTranslation";
import type { AppSettings } from "../lib/tauri";

// macOS keeps monitor capture without masking in this version
// (privacy-capture D10). WKWebView reports "Macintosh"; WebView2 never does.
const IS_MAC = navigator.userAgent.includes("Mac");

/** The full privacy text (review-redaction D7). */
export const PRIVACY_URL = "https://github.com/wizi89/sop-recorder/blob/main/docs/PRIVACY.md";

/** The settings section "Datenschutz bei der Aufnahme": capture area and password masking. */
export function CaptureAreaSettings({
  settings,
  setSettings,
  loaded,
}: {
  settings: AppSettings;
  setSettings: Dispatch<SetStateAction<AppSettings>>;
  loaded: boolean;
}) {
  const { t } = useTranslation();
  return (
    <>
      <div className="flex items-center justify-between">
        <label className="label-sm" htmlFor="capture-mode">
          {t("settings.capture_area")}
        </label>
        <select
          id="capture-mode"
          value={IS_MAC ? "monitor" : settings.capture_mode}
          disabled={!loaded || IS_MAC}
          onChange={(e) => setSettings((s) => ({ ...s, capture_mode: e.target.value as AppSettings["capture_mode"] }))}
          className="bg-surface-container-highest text-on-background rounded-lg px-3 py-2 text-sm outline-none"
        >
          {!IS_MAC && (
            <option value="active_window">{t("settings.capture_active_window")}</option>
          )}
          <option value="monitor">{t("settings.capture_monitor")}</option>
        </select>
      </div>
      <p
        className="text-on-surface-variant leading-snug"
        style={{ fontSize: "0.625rem", marginTop: "-0.75rem" }}
      >
        {IS_MAC
          ? t("settings.capture_mac_note")
          : settings.capture_mode === "monitor"
            ? t("settings.capture_monitor_warning")
            : t("settings.capture_active_window_hint")}
      </p>

      {!IS_MAC && (
        <div className="flex items-center justify-between">
          <label className="label-sm">{t("settings.mask_passwords")}</label>
          <button
            className="switch-track"
            role="switch"
            aria-checked={settings.mask_passwords}
            aria-label={t("settings.mask_passwords")}
            data-checked={settings.mask_passwords}
            disabled={!loaded}
            onClick={() =>
              setSettings((s) => ({ ...s, mask_passwords: !s.mask_passwords }))
            }
          >
            <span
              className="switch-thumb"
              style={{ left: settings.mask_passwords ? 21 : 3 }}
            />
          </button>
        </div>
      )}
      {!IS_MAC && !settings.mask_passwords && (
        <p
          className="text-on-surface-variant leading-snug"
          style={{ fontSize: "0.625rem", marginTop: "-0.75rem" }}
        >
          {t("settings.mask_passwords_off")}
        </p>
      )}
      <p
        className="text-on-surface-variant leading-snug"
        style={{ fontSize: "0.625rem", marginTop: "-0.75rem" }}
      >
        {t("settings.privacy_summary")}{" "}
        <button
          type="button"
          onClick={() => void openUrl(PRIVACY_URL)}
          className="text-primary hover:underline bg-transparent border-none cursor-pointer p-0"
          style={{ fontSize: "inherit" }}
        >
          {t("settings.privacy_link")}
        </button>
      </p>
    </>
  );
}
