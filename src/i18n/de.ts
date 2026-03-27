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
  "status.stopping": "Aufnahme wird gestoppt...",
  "status.copying": "Screenshots werden kopiert...",
  "status.transcribing": "Audio wird transkribiert...",
  "status.generating": "Anleitung wird mit KI erstellt...",
  "status.generating_pdf": "PDF wird erstellt...",
  "status.uploading_server": "Wird an Server gesendet",
  "status.uploading_webapp": "Wird in Web-App hochgeladen...",
  "status.done_uploaded": "Fertig! Gespeichert und hochgeladen",
  "status.done_local_upload_failed":
    "Fertig! Lokal gespeichert (Upload fehlgeschlagen)",
  "status.done": "Fertig! Anleitung gespeichert",
  "status.no_clicks": "Keine Klicks aufgezeichnet. Nichts zu verarbeiten.",
  "status.sign_in_required":
    "Anmeldung erforderlich für Server-Verarbeitung",
  "status.open_folder": "Ordner öffnen",
  "status.retry": "Erneut versuchen",
  "status.pending_found": "Ausstehend: {folder}",

  // Settings
  "settings.title": "Einstellungen",
  "settings.model": "Modell",
  "settings.api_key": "API-Schlüssel",
  "settings.hide_screenshots": "In Screenshots ausblenden",
  "settings.skip_pii_check": "PII-Prüfung überspringen",
  "settings.workflows_dir": "Anleitungsverzeichnis",
  "settings.logs_dir": "Protokollverzeichnis",
  "settings.choose": "Wählen",
  "settings.upload_to": "Hochladen an",
  "settings.save": "Speichern",
  "settings.cancel": "Abbrechen",

  // Network errors
  "network.session_expired":
    "Sitzung abgelaufen. Bitte abmelden und erneut anmelden.",
  "network.connection_failed":
    "Server nicht erreichbar. Prüfe die Verbindung und versuche es erneut.",
  "network.server_closed":
    "Server hat die Verbindung geschlossen, ohne ein Ergebnis zu senden.",
  "network.signin_failed": "Anmeldung fehlgeschlagen",
  "network.pii_blocked":
    "Personenbezogene Daten erkannt. Erzeugung abgebrochen.",
  "network.pii_step": "Schritt {step}",
  "network.pii_transcript": "Audiotranskript",

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
