# Datenschutz bei der Aufnahme

Was der Recorder bei einer Aufnahme festhält, was davon an den Server geht, was geprüft wird und
was nicht. Stand: die Version mit „Aktives Fenster“, „Passwortfelder schwärzen“ und den
Schwärzungen auf dem Prüfbildschirm. Ältere Versionen kennen weder die Schwärzungen noch die
Prüfmarkierungen.

## Was aufgenommen wird

- **Ein Screenshot pro Schritt:** bei jedem Klick und bei der Eingabetaste.
  - **Windows, „Aktives Fenster“ (Standard):** nur das Fenster, in dem geklickt wird, zusammen
    mit seinen Dialogen und Menüs. Taskleiste und andere Fenster sind nicht zu sehen, außer
    bei einem Klick auf die Taskleiste: dann nur diese.
  - **Klick auf den Desktop:** der ganze Bildschirm, wenn sich kein aktives Fenster bestimmen
    lässt. Solche Schritte werden zur Prüfung markiert (siehe unten).
  - **„Ganzer Bildschirm“:** der Bildschirm, auf dem geklickt wird, mit Taskleiste,
    Benachrichtigungen und allen sichtbaren Fenstern.
  - **macOS:** immer der ganze Bildschirm, ohne Schwärzen von Passwortfeldern.
- **Ton:** das Mikrofon, während die Aufnahme läuft.
- **Pro Schritt:** Zeitpunkt, Klickposition, Auslöser (Klick oder Eingabetaste), der
  Aufnahmebereich, das Ergebnis der Passwortprüfung und ob die Liste der offenen Fenster
  unvollständig war.

Nicht aufgenommen werden getippter Text (nur die Eingabetaste wird erkannt), Namen von
Bedienelementen, andere Maustasten und das Scrollen.

## Was vor dem Speichern geschwärzt wird (Windows)

- **Passwortfelder**, wenn „Passwortfelder schwärzen“ eingeschaltet ist (Standard). Das gilt
  auch für Felder, in die nicht geklickt wurde. Gefundene Felder werden geschwärzt, bevor der
  Screenshot auf die Festplatte geschrieben wird. Es entsteht also keine ungeschwärzte Kopie
  eines gefundenen Feldes.
- **Fenster anderer Programme**, die über dem aufgenommenen Fenster liegen, in der Einstellung
  „Aktives Fenster“: Benachrichtigungen, Info-Center, Schnelleinstellungen, Fenster, die
  immer im Vordergrund sind, Kontextmenüs anderer Programme und Vorschaubilder der Taskleiste. Sie werden dort geschwärzt, wo sie das Fenster überdecken.
  Einblendungen, durch die man hindurchklicken kann (etwa eine Diktierleiste), bleiben sichtbar.
- **Lücken:** Liegt ein Dialog weit entfernt von seinem Hauptfenster, wird die Fläche zwischen
  beiden geschwärzt.

## Was geprüft wird und was nicht

**Geprüft:**

- **Passwortfelder auf Windows**, wenn das Schwärzen eingeschaltet ist.
  - Lässt sich ein Schritt nicht sicher prüfen, markiert der Prüfbildschirm ihn mit „Prüfen“:
    zum Beispiel eine Prüfung, die zu lange dauert.
  - Ebenso markiert werden Schritte, bei denen statt des aktiven Fensters der ganze Bildschirm
    aufgenommen wurde, Schritte, bei denen die Liste der offenen Fenster unvollständig war
    (dann ist eine Benachrichtigung oder ein anderes Fenster darüber vielleicht nicht geschwärzt; mit und ohne
    Schwärzen von Passwortfeldern), und Schritte ohne Aufzeichnung, wie sie aufgenommen wurden
    (Aufnahmen älterer Versionen).
  - „Erstellen“ geht erst, wenn „Ich habe die markierten Schritte geprüft“ angehakt ist.
- **Personenbezogene Daten auf dem Server:** Transkript und Bildschirmtext werden dort geprüft.
  Der Server kann die Erstellung dann ablehnen. Diese Prüfung lässt sich in den Einstellungen
  abschalten.

**Nicht geprüft:**

- **Der Inhalt des Tons:** Was Sie sagen, wird gesendet.
- **Alles andere, was im aufgenommenen Fenster sichtbar ist:** Namen, E-Mail-Adressen, Kunden-
  oder Kontodaten und Text in gewöhnlichen Eingabefeldern bleiben sichtbar, solange Sie sie
  nicht selbst schwärzen.
- **Sichtbar geschaltete Passwörter:** Ein Passwort, das mit „Passwort anzeigen“ sichtbar
  gemacht wurde, ist kein Passwortfeld mehr und wird nicht geschwärzt.
- **„Ganzer Bildschirm“:** Fenster anderer Programme werden nicht geschwärzt, nur Passwortfelder.
- **macOS:** Dort wird nichts automatisch geschwärzt.

**Nicht aufgenommen:** Klicks und die Eingabetaste in Programmen, die mit Administratorrechten
laufen, werden nicht aufgenommen, auch nicht in Installationsprogrammen und in der
Windows-Sicherheitsabfrage. Von dort gelangt nichts in die Aufnahme. Solange ein solches Fenster
vorne ist, sagt es die Aufnahmeleiste. Der Prüfbildschirm zeigt an der Stelle, an der es
geschah, „Nicht aufgenommen“ mit den Namen der Programme (nicht den Fenstertiteln). Diese Angabe
bleibt auf Ihrem PC und wird nicht hochgeladen; was Sie in dieser Zeit sagen, wird wie bisher
mit dem Ton gesendet.

## Schwärzen und Löschen auf dem Prüfbildschirm

- **Schwärzen:** Klicken Sie einen Schritt an, um ihn vergrößert zu öffnen. Ziehen Sie dort
  einen Rahmen auf („Schwärzen“). Die Rahmen werden beim Zeichnen gespeichert, überstehen einen
  Neustart und lassen sich bis zum Erstellen wieder entfernen.
- **Wann die Rahmen wirken:** Sie werden erst beim Hochladen in eine Kopie eingebrannt. Die
  Screenshot-Dateien auf Ihrem PC bleiben unverändert. Kann ein Rahmen nicht angewendet werden,
  wird nichts hochgeladen, und Sie kommen zurück zum Prüfbildschirm.
- **Spätere Rahmen:** Rahmen wirken nur auf Uploads, die nach dem Zeichnen stattfinden. Eine
  Anleitung, die vorher erstellt wurde, behält ihre Screenshots auf dem Server, bis sie dort
  gelöscht wird.
- **„Schritt löschen“:** Entfernt den Screenshot und seine Angaben. Der Ton wird nicht
  geschnitten. Was Sie während dieses Schritts gesagt haben, wird mitgesendet und kann im Text
  eines benachbarten Schritts erscheinen.

## Was den Rechner verlässt

Beim Erstellen (und nur dann) werden an den CogniClone-Server gesendet:

- **Die Screenshots:** jeder nicht gelöschte Screenshot, so wie er gespeichert wurde, also
  zugeschnitten und mit geschwärzten Passwortfeldern und fremden Fenstern, dazu mit den
  eingebrannten Rahmen vom Prüfbildschirm.
- **Der Ton** der ganzen Aufnahme.
- **Die Angaben pro Schritt** (siehe oben) und die Zahl der Schritte je Aufnahmebereich und je
  Ergebnis der Passwortprüfung, dazu die Zahl der Schritte mit unvollständiger Fensterliste.
- **Titel, Pipeline-Auswahl und Modell.**

Während der Aufnahme wird nichts gesendet. Was sonst gesendet wird, etwa ein Fehlerbericht nur
nach Ihrer Zustimmung, steht in [ARCHITECTURE.md](./ARCHITECTURE.md#what-leaves-the-machine).

## Was auf Ihrem Rechner bleibt

Der Aufnahme-Ordner enthält:

- den Ton,
- die Screenshots, mit geschwärzten Passwortfeldern und fremden Fenstern, aber **ohne** die
  Rahmen vom Prüfbildschirm,
- die Angaben pro Schritt,
- die gezeichneten Rahmen (`redactions.json`).

Die Rahmen wendet nur dieser Recorder beim Hochladen an. Andere Programme, die den Ordner öffnen,
und ältere Versionen des Recorders sehen die Screenshots ohne Rahmen. Den Ordner verwalten Sie
selbst; der Recorder löscht ihn nicht.
