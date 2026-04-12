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
  "status.retry_from_disk": "Aus Aufnahme erneut versuchen",
  "status.undo_last": "Letzten Schritt entfernen",
  "status.stopping": "Aufnahmen werden verarbeitet...",

  // Review screen (post-stop, pre-generate)
  "review.title": "Aufnahme prüfen",
  "review.summary": "{count} Screenshots -- {elapsed} Min aufgenommen",
  "review.loading": "Screenshots werden geladen...",
  "review.step_label": "Schritt {n}",
  "review.confirm": "Generieren",
  "review.cancel": "Verwerfen",

  // Microphone permission
  "mic.permission_denied":
    "Mikrofon-Zugriff verweigert -- in den Systemeinstellungen erlauben",

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
    "CogniClone hat sensible Daten in deiner Aufnahme gefunden und die Verarbeitung gestoppt.",
  "pii.settings_hint":
    "Du kannst diese Prüfung in den Einstellungen deaktivieren.",
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
  "pii.dismiss": "Schließen",
  "pii.link_legal": "Rechtliches",
  "pii.link_privacy": "Datenschutz",
  "pii.link_terms": "AGB",

  // PII disable confirmation (settings)
  "pii.confirm_title": "Sicherheitsprüfung deaktivieren?",
  "pii.confirm_intro":
    "Du bist dabei, die automatische Sicherheitsprüfung zu deaktivieren.",
  "pii.confirm_explain":
    "Standardmäßig prüft CogniClone deine Aufnahmen automatisch auf personenbezogene Daten, Passwörter und sensible Inhalte, bevor sie an die KI weitergegeben werden. Wenn du diesen Filter deaktivierst, werden Aufnahmen direkt und ungefiltert zur KI-Verarbeitung übermittelt.",
  "pii.confirm_bullet_1":
    "Keine automatische Erkennung von Namen, E-Mail-Adressen, Telefonnummern oder Zugangsdaten",
  "pii.confirm_bullet_2":
    "Keine automatische Erkennung von Passwörtern oder API-Schlüsseln",
  "pii.confirm_bullet_3":
    "Keine automatische Erkennung vertraulicher Unternehmensinhalte",
  "pii.confirm_responsibility":
    "Du übernimmst damit die vollständige Verantwortung dafür, dass deine Aufnahmen keine personenbezogenen Daten Dritter oder sensiblen Informationen enthalten, für die du keine Rechtsgrundlage zur Verarbeitung hast.",
  "pii.confirm_scope":
    "Diese Einstellung betrifft alle zukünftigen Aufnahmen in deinem Account, bis du den Filter wieder aktivierst.",
  "pii.confirm_accept": "Verstanden, deaktivieren",
  "pii.confirm_cancel": "Abbrechen",

  // PII disabled chip (main screen)
  "pii.disabled_chip": "Sicherheitsprüfung deaktiviert",

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

  // Quota / rate limit
  "quota.used": "{count} / {limit} Anleitungen",
  "quota.exceeded_title": "Kontingent ausgeschöpft",
  "quota.exceeded_message":
    "Du hast dein Kontingent von {limit} Anleitungen vollständig genutzt. Für weitere Aufnahmen benötigst du ein höheres Kontingent.",
  "quota.exceeded_message_generic":
    "Dein Kontingent für Anleitungen ist ausgeschöpft. Für weitere Aufnahmen benötigst du ein höheres Kontingent.",
  "quota.exceeded_upgrade": "Kontingent erweitern",
  "quota.exceeded_dismiss": "Schließen",

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
