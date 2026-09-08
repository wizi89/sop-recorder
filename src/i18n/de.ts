const de = {
  // Login
  "login.email": "E-Mail",
  "login.password": "Passwort",
  "login.sign_in": "Anmelden",
  "login.signing_in": "Anmeldung...",
  "login.enter_credentials": "Bitte E-Mail und Passwort eingeben.",
  "login.forgot_password": "Passwort vergessen?",
  "login.create_account": "Konto erstellen",
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
  "status.elapsed": "Läuft seit {elapsed} Min",
  "status.still_waiting":
    "Der Server arbeitet noch. Das Fenster kann offen bleiben, das Ergebnis kommt automatisch.",
  "status.retry": "Erneut versuchen",
  "status.retry_from_disk": "Aus Aufnahme erneut versuchen",
  "status.generate_from_folder": "Aus Ordner generieren",
  "status.invalid_recording_folder":
    "Dieser Ordner enthält keine Aufnahme. Bitte einen Ordner mit einer Aufzeichnung (recording.wav und screenshots/) wählen.",
  "status.undo_last": "Letzten Schritt entfernen",
  "status.stopping": "Aufnahmen werden verarbeitet...",

  // Review screen (post-stop, pre-generate)
  "review.title": "Aufnahme prüfen",
  "review.summary": "{count} Screenshots -- {elapsed} Min aufgenommen",
  "review.summary_count_only": "{count} Screenshots in diesem Ordner",
  "review.failed_captures":
    "{count} von {total} Schritten konnten nicht aufgenommen werden. Die Anleitung ist unvollständig.",
  "review.loading": "Screenshots werden geladen...",
  "review.step_label": "Schritt {n}",
  "review.confirm": "Generieren",
  "review.cancel": "Verwerfen",
  "review.pipeline_label": "Art der Anleitung",
  "review.pipeline_default": "Standard",
  "review.pipeline_default_description":
    "Die allgemeine Generierung. Passt zu jeder Aufnahme.",

  // Microphone permission
  "mic.permission_denied":
    "Mikrofon-Zugriff verweigert -- in den Systemeinstellungen erlauben",

  // Permission bootstrap (macOS TCC)
  "permissions.screen_recording_denied":
    "Bildschirmaufnahme verweigert -- ohne diese Berechtigung sieht CogniClone nur den Desktop",
  "permissions.accessibility_denied":
    "Bedienungshilfen verweigert -- ohne diese Berechtigung erkennt CogniClone keine Klicks und zeichnet keine Schritte auf",
  "permissions.grant":
    "Erlauben",

  // First-run permission screen
  "permissions.setup_title": "Berechtigungen erteilen",
  "permissions.setup_intro":
    "CogniClone benötigt drei Berechtigungen, um eine Anleitung aufzeichnen zu können. Du erteilst sie einmal -- danach fragt die App nicht wieder.",
  "permissions.mic_title": "Mikrofon",
  "permissions.mic_why":
    "Zeichnet deine gesprochene Erklärung auf, aus der die Beschreibung der Schritte entsteht.",
  "permissions.screen_title": "Bildschirmaufnahme",
  "permissions.screen_why":
    "Erstellt bei jedem Klick ein Bildschirmfoto, das den jeweiligen Schritt zeigt.",
  "permissions.accessibility_title": "Bedienungshilfen",
  "permissions.accessibility_why":
    "Erkennt Klicks und Tastendrücke, damit CogniClone weiß, wann ein neuer Schritt beginnt.",
  "permissions.state_granted": "Erteilt",
  "permissions.state_denied": "Fehlt",
  "permissions.state_unknown": "Wird geprüft",
  "permissions.grant_all": "Alle Berechtigungen erteilen",
  "permissions.restart_hint":
    "Bildschirmaufnahme und Bedienungshilfen werden erst nach einem Neustart der App wirksam -- das verlangt macOS.",
  "permissions.restart": "App neu starten",
  "permissions.settings_hint":
    "Mikrofon und Bedienungshilfen erkennt CogniClone sofort. Die Bildschirmaufnahme prüft macOS nur beim Start, dafür ist ein Neustart nötig.",
  "permissions.skip": "Später erteilen",
  "permissions.all_granted": "Alle Berechtigungen sind erteilt.",
  "permissions.grant_to_start":
    "Berechtigungen erteilen, um eine Aufnahme zu starten",

  // Settings
  "settings.title": "Einstellungen",
  "settings.hide_screenshots": "In Screenshots ausblenden",
  "settings.skip_pii_check": "PII-Prüfung überspringen",
  "settings.reveal": "Anzeigen",
  "settings.loading": "Wird geladen...",
  "settings.save_failed":
    "Einstellungen wurden nicht gespeichert: {error}. Das Fenster bleibt offen, damit nichts verloren geht.",
  "settings.workflows_dir": "Anleitungsverzeichnis",
  "settings.logs_dir": "Protokollverzeichnis",
  "settings.choose": "Wählen",
  "settings.upload_to": "Hochladen an",
  "settings.pipeline_label": "Pipeline-Version",
  "settings.model_label": "KI-Modell",
  "settings.save": "Speichern",
  "settings.cancel": "Abbrechen",
  "settings.error_reports": "Fehlerberichte",
  "settings.error_reports_ask": "Vor dem Senden fragen",
  "settings.error_reports_always": "Automatisch senden",
  "settings.error_reports_never": "Keine Fehlerberichte",
  "settings.error_reports_hint":
    "Wenn CogniClone abstürzt oder ein Fehler auftritt, kann ein Bericht an uns gesendet werden. Er enthält nie Aufnahmen, Screenshots oder deine E-Mail-Adresse.",
  "settings.error_reports_disabled_by_org": "Von Ihrer Organisation deaktiviert",

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
  "pii.link_legal": "Impressum",
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

  // Error reports
  "report.title": "Fehlerbericht senden?",
  "report.title_crash": "CogniClone wurde beim letzten Mal unerwartet beendet",
  "report.intro":
    "In CogniClone ist ein Fehler aufgetreten. Ein Bericht darüber hilft uns, ihn zu beheben.",
  "report.intro_crash":
    "Beim letzten Start hat sich CogniClone unerwartet beendet. Ein Bericht darüber hilft uns, den Absturz zu beheben.",
  "report.consent": "Der Bericht wird nur gesendet, wenn du zustimmst.",
  "report.contains_title": "Der Bericht enthält:",
  "report.contains_1": "die Fehlermeldung und die Stelle im Programm, an der sie entstand",
  "report.contains_2": "App-Version, Betriebssystem und Sprache",
  "report.contains_3":
    "die letzten Protokollzeilen, ohne Dateinamen, Pfade und Adressen",
  "report.contains_4": "deine Einstellungen zur Verarbeitung",
  "report.excludes_title": "Der Bericht enthält nie:",
  "report.excludes_1": "Screenshots, Ton oder Transkripte",
  "report.excludes_2": "Inhalte deiner Anleitungen",
  "report.excludes_3": "deine E-Mail-Adresse, Zugangsdaten oder Schlüssel",
  "report.excludes_4": "den Namen oder Pfad deines Anleitungsverzeichnisses",
  "report.show_details": "Details anzeigen",
  "report.hide_details": "Details ausblenden",
  "report.comment_label": "Was hast du gerade gemacht? (optional)",
  "report.comment_placeholder": "Zum Beispiel: Ich habe auf Generieren geklickt.",
  "report.always_send": "Fehlerberichte künftig automatisch senden",
  "report.send": "Bericht senden",
  "report.signed_out_hint":
    "Du bist nicht angemeldet. Der Bericht wird gespeichert und nach der nächsten Anmeldung gesendet.",
  "report.reveal_file": "Bericht-Datei anzeigen",
  "report.decline": "Nicht senden",
  "report.sent_title": "Danke -- der Bericht ist angekommen.",
  "report.sent_number": "Berichtsnummer: {number}",
  "report.sent_hint":
    "Nenne uns diese Nummer, wenn du dich zu dem Fehler bei uns meldest.",
  "report.copy": "Kopieren",
  "report.copied": "Kopiert!",
  "report.close": "Schließen",
  "report.send_button": "Fehlerbericht senden",
  "report.auto_sent": "Fehlerbericht gesendet -- Nummer {number}",

  // Tray
  "tray.show": "Anzeigen",
  "tray.hide": "Ausblenden",
  "status.recording_in_progress":
    "Aufnahme läuft -- die Steuerleiste liegt unten rechts auf dem Bildschirm.",
  "permissions.blocked_start":
    "Aufnahme nicht möglich: {names} fehlt. Ohne diese Berechtigung entsteht keine brauchbare Anleitung.",
  "permissions.no_mic_title": "Ohne Mikrofon aufnehmen?",
  "permissions.no_mic_confirm":
    "Die Mikrofon-Berechtigung fehlt. Die Schritte werden aufgezeichnet, aber ohne gesprochene Erklärung fällt die Beschreibung deutlich knapper aus.",
  "permissions.no_mic_continue": "Trotzdem aufnehmen",
  "permissions.grant_one": "Erteilen",
  "permissions.add_manually":
    "CogniClone steht dort meist nicht von selbst in der Liste -- dann unten mit + aus dem Programme-Ordner hinzufügen.",
  "permissions.needs_restart":
    "Diese Berechtigung erkennt CogniClone erst nach einem Neustart der App.",
  "permissions.restart_now": "App neu starten",
  "permissions.remaining_hint":
    "Diese Berechtigungen hat macOS bereits einmal abgefragt. Sie lassen sich nur noch in den Systemeinstellungen ändern.",
  "permissions.state_undetermined": "Noch nicht erteilt",
  "permissions.open_settings": "In Systemeinstellungen öffnen",
  "permissions.denied_hint":
    "Einmal verweigerte Berechtigungen fragt macOS nicht erneut ab -- sie müssen in den Systemeinstellungen erteilt werden.",
  "status.no_audio": "Kein Ton",
  "status.no_audio_hint":
    "Es kommt kein Ton an. Vermutlich fehlt die Mikrofon-Berechtigung -- diese Aufnahme bekommt keine Sprachbeschreibung.",
  "tray.start_recording": "Aufnahme starten",
  "tray.stop_recording": "Aufnahme stoppen",
  "tray.settings": "Einstellungen",
  "tray.quit": "Beenden",
} as const;

export type TranslationKey = keyof typeof de;
export default de;
