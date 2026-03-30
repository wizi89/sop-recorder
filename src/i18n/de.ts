const de = {
  // Login
  "login.email": "E-Mail",
  "login.password": "Passwort",
  "login.sign_in": "Anmelden",
  "login.signing_in": "Anmeldung...",
  "login.enter_credentials": "Bitte E-Mail und Passwort eingeben.",
  "login.forgot_password": "Passwort vergessen?",
  "login.sign_out": "Abmelden",

  // Recording
  "status.ready": "Bereit",
  "status.start": "Aufnahme starten",
  "status.stop": "Aufnahme stoppen",
  "status.done_uploaded": "Fertig! Gespeichert und hochgeladen",
  "status.no_clicks": "Keine Screenshots aufgezeichnet. Bitte erneut versuchen.",
  "status.cancel": "Abbrechen",
  "status.cancel_title": "Aufnahme abbrechen?",
  "status.cancel_message": "Die aktuelle Aufnahme wird verworfen.",
  "status.cancel_confirm": "Verwerfen",
  "status.open_folder": "Ordner öffnen",
  "status.retry": "Erneut versuchen",

  // Settings
  "settings.title": "Einstellungen",
  "settings.hide_screenshots": "In Screenshots ausblenden",
  "settings.skip_pii_check": "PII-Prüfung überspringen",
  "settings.workflows_dir": "Anleitungsverzeichnis",
  "settings.logs_dir": "Protokollverzeichnis",
  "settings.choose": "Wählen",
  "settings.upload_to": "Hochladen an",
  "settings.save": "Speichern",
  "settings.cancel": "Abbrechen",

  // PII blocked modal
  "pii.title": "Personenbezogene Daten erkannt",
  "pii.message":
    "In Ihrer Aufnahme wurden personenbezogene Daten erkannt. Die Erzeugung wurde abgebrochen.",
  "pii.settings_hint":
    "Sie können die PII-Prüfung in den Einstellungen deaktivieren und es erneut versuchen.",
  "pii.disclaimer":
    "Mit dem Deaktivieren der PII-Prüfung bestätigen Sie, dass Ihre Daten ohne automatische Filterung personenbezogener Daten verarbeitet werden.",
  "pii.source_step": "Schritt {step}",
  "pii.source_transcript": "Audiotranskript",
  "pii.entity_EMAIL_ADDRESS": "E-Mail-Adresse",
  "pii.entity_PHONE_NUMBER": "Telefonnummer",
  "pii.entity_IBAN_CODE": "IBAN",
  "pii.entity_CREDIT_CARD": "Kreditkarte",
  "pii.entity_IP_ADDRESS": "IP-Adresse",
  "pii.entity_DE_STEUER_ID": "Steuer-ID",
  "pii.entity_DE_SOZIALVERSICHERUNGSNUMMER": "Sozialversicherungsnr.",
  "pii.entity_DE_PERSONALAUSWEIS": "Personalausweisnr.",
  "pii.copy": "Kopieren",
  "pii.copied": "Kopiert!",
  "pii.link_legal": "Rechtliches",
  "pii.link_privacy": "Datenschutz",
  "pii.link_terms": "AGB",
  "pii.dismiss": "Zurück",

  // Network errors
  "network.pii_blocked":
    "Personenbezogene Daten erkannt. Erzeugung abgebrochen.",
  "network.session_expired":
    "Sitzung abgelaufen. Bitte abmelden und erneut anmelden.",
  "network.connection_failed":
    "Server nicht erreichbar. Prüfe die Verbindung und versuche es erneut.",
  "network.server_closed":
    "Server hat die Verbindung geschlossen, ohne ein Ergebnis zu senden.",
  "network.signin_failed": "Anmeldung fehlgeschlagen",

  // PDF
  "pdf.steps": "{count} Schritte",
  "pdf.step": "Schritt {order}",
  "pdf.no_title": "(ohne Titel)",

  // Update
  "update.available": "Version {version} verfügbar",
  "update.install": "Jetzt aktualisieren",
  "update.downloading": "Wird heruntergeladen...",

  // Errors
  "error.prefix": "Fehler: {message}",

  // Tray
  "tray.show": "Anzeigen",
  "tray.hide": "Ausblenden",
  "tray.start_recording": "Aufnahme starten",
  "tray.stop_recording": "Aufnahme stoppen",
  "tray.settings": "Einstellungen",
  "tray.quit": "Beenden",
} as const;

export type TranslationKey = keyof typeof de;
export default de;
