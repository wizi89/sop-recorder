import { describe, it, expect } from "vitest";
import { t } from "../i18n";
import de from "../i18n/de";

describe("i18n", () => {
  it("returns the German translation for known keys", () => {
    expect(t("login.email")).toBe("E-Mail");
    expect(t("login.password")).toBe("Passwort");
    expect(t("status.ready")).toBe("Bereit");
    expect(t("status.start")).toBe("Aufnahme starten");
    expect(t("status.stop")).toBe("Aufnahme stoppen");
  });

  it("substitutes placeholders", () => {
    expect(t("pdf.steps", { count: 5 })).toBe("5 Schritte");
    expect(t("pdf.step", { order: 3 })).toBe("Schritt 3");
    expect(t("status.pending_found", { folder: "SOP 2026-01-01" })).toBe(
      "Ausstehend: SOP 2026-01-01",
    );
  });

  it("returns the key itself for unknown keys", () => {
    // @ts-expect-error testing unknown key
    expect(t("nonexistent.key")).toBe("nonexistent.key");
  });

  it("has all required UI keys", () => {
    const requiredKeys = [
      // Login
      "login.email",
      "login.password",
      "login.sign_in",
      "login.signing_in",
      "login.forgot_password",
      "login.sign_out",
      // Recording
      "status.ready",
      "status.start",
      "status.stop",
      "status.done",
      "status.open_folder",
      "status.retry",
      // Settings
      "settings.title",
      "settings.hide_screenshots",
      "settings.workflows_dir",
      "settings.logs_dir",
      "settings.save",
      "settings.cancel",
      // Tray
      "tray.show",
      "tray.hide",
      "tray.quit",
      // Errors
      "network.connection_failed",
      "network.session_expired",
    ];

    for (const key of requiredKeys) {
      expect(de).toHaveProperty(key);
      expect((de as Record<string, string>)[key]).toBeTruthy();
    }
  });

  it("uses proper German umlauts (not ASCII substitutes)", () => {
    // Spot check specific strings that must have umlauts
    expect(t("status.open_folder")).toContain("ö"); // Ordner offnen -> Ordner öffnen
    expect(t("settings.workflows_dir")).not.toContain("ue"); // no ASCII substitutes
  });
});
