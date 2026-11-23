# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Password fields are blacked out in the screenshots on Windows, including fields you did not click. A field is filled in before the screenshot is written, so no unmasked copy of a found field is saved. When the check cannot confirm a step, the step is marked for review (below). A password made visible with a "show password" button is no longer a password field and is not blacked out.
- The review screen marks steps with "Prüfen" when a password field could not be checked reliably, when the active window could not be found and the whole screen was captured instead, or when the list of open windows was incomplete, so a notification over the captured window may not be blacked out (with "Passwortfelder schwärzen" on or off). A recording made with an earlier version has every step marked. Generating needs the tick "Ich habe die markierten Schritte geprüft" first.
- On the review screen, click a step to open it enlarged. Drag a frame to black out an area ("Schwärzen"), or delete the step ("Schritt löschen"). Boxes are saved as you draw them, survive a restart, and can be removed until you generate. They are burned into the uploaded copy only: the screenshot files on your PC stay the originals. If a box cannot be applied, nothing is uploaded and you are taken back to the review screen. Deleting a step keeps the other steps' numbers and does not cut the audio, so what you said during that step is still sent. Press Esc or "Fertig" to close the enlarged step.
- Under "Einstellungen", a short note says what is sent when you generate, with a link to the full privacy text.

### Changed

- On Windows, a screenshot now shows only the window you clicked in, together with its dialogs and menus, instead of the whole screen. The taskbar and other windows are no longer captured. Where a notification, the notification center, quick settings or another app's window lies over the captured one, that part is blacked out; overlays that let clicks through, such as a dictation bar, stay visible.
- To go back to the old behaviour, open "Einstellungen", set "Aufnahmebereich" to "Ganzer Bildschirm". Switch off "Passwortfelder schwärzen" to stop blacking out password fields; parts of other windows lying over the captured one are blacked out either way.
- On macOS nothing changes yet: the whole screen is captured and passwords are not blacked out.
- On Windows, an update now downloads as the regular installer instead of a zip archive. Nothing changes for you; it removes a library with known security flaws from the updater.

### Fixed

- In a folder opened with "Aus Ordner generieren", a file such as `step_+3.png` is no longer taken for a step, and deleting a step also removes a second screenshot of the same number (`step_1.png` next to `step_01.png`), so it is not uploaded.
- On Windows with more than one monitor, the recording bar now moves to the monitor you click on, so it stays in sight while you work on another monitor.

## [0.20.1] - 2026-10-25

### Changed

- When no sound arrives during a recording, the bar keeps showing the number of steps and the time; "Kein Ton" now appears below them instead of replacing them.

## [0.20.0] - 2026-10-25

### Added

- A recording that was never turned into a guide -- because the app was closed on the review screen, generating failed, or the app crashed after stopping -- is offered when the app starts. You can review it and generate it, or discard it. Recordings older than a week are not offered; they are still in their folder.

### Changed

- If the microphone is missing or will not open, the recording now starts anyway and captures the steps without narration. The bar shows "Kein Ton", as it already did when the microphone permission is missing.

### Fixed

- A short server outage or update no longer signs you out. The app says the server is unreachable and keeps trying; your sign-in stays saved.
- Generating could wait forever when a screenshot never finished saving. It now waits at most 15 seconds and goes ahead with the steps that were saved.
- After an error while stopping or generating, the app could keep thinking it was still busy, and an internal error in one part of the app could make every following action fail until the app was restarted. Both are fixed.

## [0.19.3] - 2026-10-25

### Fixed

- Closing the app's window left it running in the background, and opening it again then showed nothing. Closing the window now quits the app, and the next start opens it as usual.

## [0.19.2] - 2026-10-24

### Changed

- If signing in through the browser fails -- for example because security software on your computer blocks it -- the app now creates an error report for it, following your setting under Einstellungen: it asks first, sends it on its own, or does nothing. It is sent once you are signed in, for example through "Ohne Browser anmelden". Cancelling the sign-in or declining it in the browser creates no report.
- "Ohne Browser anmelden" stays. It was announced to stay for only one more version; it now stays until the reports show it is no longer needed.

### Fixed

- On a Mac, a guide could end with "PDF generation failed" although it had been created: when the server sent no PDF, the app tried to make one itself and could not. The guide now finishes as usual -- it is on the website and saved locally as markdown -- just without the PDF file in the folder.

### Removed

- The app no longer makes its own simpler PDF when the server's PDF is missing. The PDF always comes from the server.

## [0.19.1] - 2026-10-24

### Changed

- Clearer narration in your guides: the microphone recording is now converted to the format speech recognition needs with a proper filter. Before, high-pitched sounds such as "s" and "sch", and fan or keyboard noise, could turn into distortion in the speech range and cause misheard words.

### Fixed

- Opening the app while it was already running started a second copy. It now brings the running one to the front, or the recording bar during a recording.

### Security

- Updated the library that secures the app's connection to the server, closing a flaw in how it checks the start of an encrypted connection.

## [0.19.0] - 2026-10-19

### Added

- Beta versions on request: switch on **Beta-Versionen erhalten** under Einstellungen to receive new versions before everyone else. Switching it off again keeps your current version until the next regular version is higher -- an update never goes backwards.

### Changed

- Signing in now happens in your own web browser instead of a form inside the app, so your password manager can fill it in, and if you are already signed in on the website you only confirm. The app never sees your password. The old form is still there for one more version, behind "Ohne Browser anmelden", in case the browser cannot reach the app on your computer.

### Fixed

- After an error report was sent automatically, the "Fehlerbericht gesendet" notice with its number sometimes did not appear, although the report had arrived.
- Switching screens, for example right after signing in, could make the app raise an error report about itself.

## [0.18.0] - 2026-09-14

### Changed

- The macOS version is signed by cogniclone and approved by Apple. Until now macOS warned that the app came from an unidentified developer and had to be opened through a right-click the first time; it now opens by double-clicking like any other program, and the warning is gone.
- One-off after installing this version: macOS asks again for permission to record the screen, to see your clicks and key presses, and to use the microphone, and asks once for your login password. Apple ties those permissions to who signed the program, and that has changed. Existing switches under Systemeinstellungen -> Datenschutz & Sicherheit may still show CogniClone as allowed from the previous version -- remove that entry with the minus button if the app keeps asking. Updates after this one leave your permissions alone.

## [0.17.0] - 2026-09-08

### Fixed

- A recording no longer loses every step after one that failed. If a screenshot could not be taken -- which happened under fast clicking, when several full-screen captures ran at once -- the guide stopped at that point and everything after it was silently dropped. In one test 21 clicks became a guide with a single step, with nothing on screen to say so. All the screenshots that were taken now reach the guide, and their narration with them.
- CogniClone tells you when a step could not be captured. The review screen says how many of your actions are missing before you generate the guide, instead of presenting a short recording as a complete one.
- The first save in Einstellungen takes effect. Changing a setting while the window was still loading looked like it worked and then quietly reverted, so the setting you chose was not the one that got saved -- and on a freshly installed Mac the window could take seconds to load, which made this easy to hit. The controls now wait until your settings have loaded, and a save that fails says so and keeps the window open instead of closing as though it had worked.
- "Protokollverzeichnis" in Einstellungen names the folder the log files are actually in. On macOS it named a folder that did not exist. It is now shown rather than typed, with a button to open it, because it was never a setting you could change.
- A dropped connection during processing is described as what it is. The message said the connection was interrupted, which read as lost work, for two generations that both finished normally. It now says the connection is being restored and that processing continues.

### Changed

- A step is recorded from the screen you were working on, not from all your screens at once. With two monitors both were captured side by side into one wide image and then shrunk to fit, which left each screen at a quarter of its detail -- enough that the guide named the wrong application in a test. With one monitor nothing changes.
- The red click marker is a ring instead of a filled dot, and the drawn mouse pointer is gone. Both sat on top of the thing you had just clicked; in a test the name of the button was unreadable underneath them. The ring marks the same spot and leaves the button readable.
- While a guide is being generated, CogniClone shows how long it has been working, and says so explicitly if the server has been quiet for a while.

## [0.16.0] - 2026-09-05

### Added

- CogniClone can now send a report when something goes wrong, and asks first. A crash, a step that fails with an error the app cannot explain, or a fault in its own interface each offer to send a report -- a crash that ended the app asks the next time you start it. The dialog lists in plain words what the report contains before you decide, and shows you the exact text that would be sent if you want to read it. You can add a sentence about what you were doing, and the report gets a short number to quote if you write to us about it.
- A report contains the error message, where in the program it happened, the app version, your operating system and language, the last few hundred lines of the app's own log with file names, paths and addresses removed, and the settings that decide how a recording is processed. It never contains screenshots, sound, transcripts, the content of your guides, your e-mail address, your password or keys, or the name of your guides folder. Nothing is sent unless you agree to it, and closing the dialog means no.
- Reports go to the CogniClone server you are already signed in to, and to nothing else. If a report is created while you are signed out -- when the sign-in itself is what failed -- it waits on your machine and goes out after your next sign-in; the dialog offers to show you the file so you can send it by mail instead. A report that never gets sent is deleted after 30 days.
- Under Einstellungen, "Fehlerberichte" chooses between asking every time, sending automatically, and not collecting anything at all. Asking is the default. The dialog carries the same choice as a checkbox. An organisation can switch reports off for a whole installation, and the setting then says so.

## Older releases

- [0.15](docs/changelog/0.15.md)
- [0.13 to 0.14](docs/changelog/0.13-0.14.md)
- [0.12](docs/changelog/0.12.md)
- [0.7 to 0.11](docs/changelog/0.7-0.11.md)

[Unreleased]: https://github.com/wizi89/sop-recorder/compare/v0.20.1...HEAD
[0.20.1]: https://github.com/wizi89/sop-recorder/compare/v0.20.0...v0.20.1
[0.20.0]: https://github.com/wizi89/sop-recorder/compare/v0.19.3...v0.20.0
[0.19.3]: https://github.com/wizi89/sop-recorder/compare/v0.19.2...v0.19.3
[0.19.2]: https://github.com/wizi89/sop-recorder/compare/v0.19.1...v0.19.2
[0.19.1]: https://github.com/wizi89/sop-recorder/compare/v0.19.0...v0.19.1
[0.19.0]: https://github.com/wizi89/sop-recorder/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/wizi89/sop-recorder/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/wizi89/sop-recorder/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/wizi89/sop-recorder/compare/v0.15.0...v0.16.0
