# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.0] - 2026-09-05

### Added

- CogniClone can now send a report when something goes wrong, and asks first. A crash, a step that fails with an error the app cannot explain, or a fault in its own interface each offer to send a report -- a crash that ended the app asks the next time you start it. The dialog lists in plain words what the report contains before you decide, and shows you the exact text that would be sent if you want to read it. You can add a sentence about what you were doing, and the report gets a short number to quote if you write to us about it.
- A report contains the error message, where in the program it happened, the app version, your operating system and language, the last few hundred lines of the app's own log with file names, paths and addresses removed, and the settings that decide how a recording is processed. It never contains screenshots, sound, transcripts, the content of your guides, your e-mail address, your password or keys, or the name of your guides folder. Nothing is sent unless you agree to it, and closing the dialog means no.
- Reports go to the CogniClone server you are already signed in to, and to nothing else. If a report is created while you are signed out -- when the sign-in itself is what failed -- it waits on your machine and goes out after your next sign-in; the dialog offers to show you the file so you can send it by mail instead. A report that never gets sent is deleted after 30 days.
- Under Einstellungen, "Fehlerberichte" chooses between asking every time, sending automatically, and not collecting anything at all. Asking is the default. The dialog carries the same choice as a checkbox. An organisation can switch reports off for a whole installation, and the setting then says so.

## [0.15.0] - 2026-08-29

The recorder works on macOS. Every defect below was invisible on Windows, where the two coordinate spaces coincide, the Keychain has no analogue, and the input hook needs no permission. The last entry is the exception: a regression the macOS work introduced on Windows.

### Fixed

- The macOS app icon was Tauri's, not CogniClone's. The v0.10.0 rebrand replaced every PNG and the Windows `.ico` but missed `icon.icns`, so Windows looked correct while macOS shipped the stock icon. Regenerated from the vector source and drawn on Apple's icon grid, so it sits the same size as native icons in the Dock rather than floating as a bare mark.
- Screenshots on a Retina display captured only the top-left quarter of the screen. `Monitor` reports geometry in logical points while `capture_image` returns physical pixels, and the canvas was sized from the former and filled from the latter, so three quarters of every screenshot was discarded at a 2x scale factor. The composite is now built in physical pixels, with each monitor scaled to a single canvas so a mixed-DPI desktop still assembles correctly.
- Click markers were drawn at a fraction of their correct position, for the same reason: a click arrives in logical points and was painted into an image measured in physical pixels. A click halfway across a 2x display landed a quarter of the way in. The marker is now mapped through the geometry the image was composited against, and drawn at the canvas scale so it stays the same apparent size rather than shrinking to a speck the model cannot see.
- Pressing any key during a recording killed the application. The input listener built a human-readable name for every keyboard event inside its tap callback, via a Text Input Services call that asserts it is running on the main queue; the callback runs on its own thread, so the assertion fired and the process died on `SIGTRAP`. macOS now uses a hand-rolled event tap that reads only the button and the key code and never touches Text Input Services.
- The recorder's own control bar was composited into every screenshot, sitting on top of the thing each step was meant to document. Windows excluded it with `WDA_EXCLUDEFROMCAPTURE`; macOS now uses the direct analogue, an `NSWindow` whose sharing type is `None`, which the capture path honours.
- The compact recording bar appeared wherever the main window happened to be, usually centred over the work being recorded. The work-area query was implemented only for Windows and returned an error on macOS, which the caller quietly swallowed, leaving the window unmoved. It is now derived from the screen's visible frame, so the bar anchors to the corner as intended. It is also no longer draggable: moving it mid-recording was tempting and the press that started the drag was captured as a step.
- Stopping a recording took two clicks. The bar is always on top but is not the key window, and macOS spends the first click into an inactive window activating it rather than delivering it to the control underneath.
- Clicking the recorder's own Stop button was recorded as a step, appending a bogus final entry to every guide, and two while stopping still took two clicks. Identifying the window under the cursor from the input hook is not possible on macOS -- the APIs that could are main-thread-only and the hook runs on its own thread -- so the window now reports where it is and the check is arithmetic.
- Shipped macOS builds could not record sound at all. Tauri enables the hardened runtime for release bundles, under which reaching the microphone requires the `com.apple.security.device.audio-input` entitlement, and none was declared. macOS did not prompt and did not fail: it refused the request outright and every recording came out as digital silence, which the old cpal probe reported as a working microphone. The entitlement is now declared, along with `com.apple.security.automation.apple-events`, which the updater needs for the same reason. This was invisible in development, where `cargo build` does not apply the hardened runtime.
- The button that fired all three permission prompts at once could destroy the permission it was asked to grant, and could never repair one. macOS presents one system dialog at a time, so a batch turned an askable microphone into a refused one without ever showing the user anything; and a dialog is only ever raised while a permission is undetermined, so pressing it about a refused one did nothing at all. Each permission now carries its own action -- one prompt at a time, and a route to its System Settings pane when asking is no longer possible. The batch request is gone from the app entirely.
- Opening the Screen Recording pane showed a list with no CogniClone in it. An app appears there only once it has actually asked for screen capture, and the request had been made only by the batch that was removed. Opening that pane now asks first, which both registers the app and raises the dialog while the state is still undetermined.
- The recorder offered to start a recording it could not make. Nothing re-read the permissions between app start and recording start, so one revoked mid-session stayed invisible: a revoked microphone until the audio warning five seconds in, and a revoked Screen Recording not at all -- it simply filled the guide with pictures of the desktop wallpaper. Screen Recording and Accessibility are now checked before a recording starts and refuse it, naming what is missing. A missing microphone asks instead of refusing, because the steps are still captured and recording without narration is a reasonable thing to want.
- The permission screen stranded anyone who granted Screen Recording. macOS reports that grant only to a freshly started process, so the row stayed red however long they waited -- and the restart button was hidden until every permission was green, which that row was preventing. The restart is now always available, and the row says that it is what macOS requires rather than leaving the wait unexplained.
- The recording bar vanished the moment any app went fullscreen, exactly when the only way to stop a recording is the bar. A fullscreen app gets a Space of its own, and always-on-top sets a window's *level*, which orders it within a Space and says nothing about which Spaces it belongs to. Space membership turns out to be fixed when a window is created, from the application's activation policy at that instant: setting the collection behaviour afterwards, raising the level, applying the `nonactivatingPanel` style bit, and hiding and re-showing the window were each measured and each made no difference. So the bar is now a window of its own, built at startup inside a momentary dip to accessory policy -- the one arrangement that works -- while the main window keeps the ordinary behaviour a document window should have. It is excluded from screenshots exactly as before; sharing type is a separate property.
- The microphone permission was reported as granted whenever a microphone was attached, on a machine that had never granted anything. The check asked cpal for the default input device and its configuration, and both are CoreAudio property queries that need no permission at all. Two consequences: the setup screen showed a tick next to a permission nobody had given, and the bundled prompt never included the microphone, so that dialog interrupted the first recording instead. macOS is now asked the question through `AVCaptureDevice`, which is the layer that knows the answer. Windows keeps the device check, having no privacy layer to consult.
- A refused permission left the setup screen showing a button that could not work. macOS presents a permission dialog only while the status is undetermined; after a refusal the request call is a silent no-op. Since both states were reported as "denied", the screen offered to ask, nothing happened, and it never dismissed. The two are now distinguished, and a refused permission gets a link straight to its System Settings pane instead of an offer to ask again.
- Stopping a recording took two clicks again once the bar became its own window: `acceptFirstMouse` is set in the window configuration, which applies only to the main window and not to one built at runtime.
- Every screenshot on a DPI-scaled Windows display was resampled up and then thrown away again. The shared canvas the Retina fix introduced sizes each monitor by multiplying its reported geometry by its DPI scale. That is right on macOS, where `Monitor` reports `CGDisplayBounds` in points, and wrong on Windows, where it reports `dmPelsWidth`, already in physical pixels. A 2560x1440 capture at 150% was Lanczos-resampled up to 3839x2160 and straight back down to 1920x1080, costing sharpness, three times the canvas memory, and roughly a second on every step. The canvas density is now measured from the captures themselves rather than read from the DPI scale, so it comes out 1.0 on Windows whatever the display setting, 2.0 on a Retina Mac, and the resample is skipped outright on any desktop whose displays agree.
- The Accessibility permission was never checked. Without it the event tap is refused outright, so the recorder showed a running timer and captured nothing at all, and the user only discovered it once the recording was over. It is now probed at startup, prompted for alongside the microphone and screen-recording prompts, and reported in the permission banner.

### Added

- A first-run permission screen replaces the three bare OS dialogs. Each permission is listed with what it is actually for, and with its own state so it is clear which one is holding things up; one button fires every prompt. It polls while open, because Screen Recording and Accessibility are granted in System Settings and nothing tells the app when that happens, so returning from System Settings updates the rows instead of leaving them stale. Once everything is granted the button becomes a restart: macOS applies those two only to a fresh process, so the restart cannot be removed, only reduced from two stumbled into to one deliberately chosen. The screen is dismissible, so a permission the user does not want to grant yet cannot lock them out of their own app.
- The recording bar says "Kein Ton" when nothing is arriving from the microphone. A denied microphone does not fail the capture on macOS -- it yields zeroed samples -- so the recording ran to completion and produced a guide with no narration and no warning anywhere. Exactly silent audio for five seconds is what distinguishes that from a quiet room; a live input always carries some noise floor.
- The permission banner lists every missing permission on its own line rather than naming a fused combination, which needed a new string for each pair and did not survive a third permission being added.
- `scripts/macos-release-cert.sh` creates one stable self-signed certificate for shipped macOS builds, and CI signs with it when the secrets are present. Without a certificate an app's code identity is its binary hash, so every update looks to macOS like a different program: users re-grant Screen Recording, Accessibility and the microphone each time, and are asked for their Keychain password again -- while the switches in System Settings still read as on, for a version that no longer exists. Measured on macOS 26: two builds with different cdhashes signed by the same certificate produce an identical designated requirement, which is what the privacy database stores. Gatekeeper still blocks the first launch, which needs notarization and a real Developer ID; when that arrives only the certificate changes.
- `scripts/macos-dev-cert.sh` creates a stable self-signed code-signing identity for local development, and `cargo run` and `cargo test` sign through it. The Keychain grants access to a code identity, and an ad-hoc dev build's identity is its binary hash, which changes on every build -- so every rebuild looked like a new program and macOS asked for the login password again. See the README for the full explanation.

### Changed

- The recording bar is a window of its own on both platforms, not the main window resized into a corner. macOS forced the split -- see the fullscreen entry above -- and Windows follows it so there is one recorder to reason about rather than two. Three things were tied to "the recorder is the main window" and moved with it: clicks on the bar are excluded from capture by the bar's own handle (on Windows that check is by `HWND`, and pointing it at the hidden main window would have made every press of Stop a captured step), the screenshot exclusion applies to the bar, and the bar keeps its taskbar button on Windows, since the main window used to provide one and losing it would leave no way back to a bar hidden behind another window. The main window is hidden while recording and shown again afterwards, at whatever size and position it had, rather than being resized and re-centred.

- Dev builds keep their credentials under a separate Keychain service from the shipped app. A dev build reading the release app's items is an untrusted caller and is challenged for the login password on every read; giving dev its own items means the dev binary creates them and is on their access list from the start. A developer's real login is left untouched, and a dev build cannot spend a release token by accident.

- The Screen Recording row explains the `+` button. macOS 15 and later require a binary signed with an Apple-issued Developer ID, specifically one carrying a Team ID, before screen capture permission registers properly; ad-hoc bundles do not qualify and neither does a self-signed certificate, since only Apple issues Team IDs. Measured both ways: the request returns, the pane opens, and no row appears either time. Adding the app by hand with `+` works and permission then functions normally, so the row presents that as the normal path rather than a fallback. This is fixed by the Developer Program enrollment and by nothing else -- it is not a matter of choosing a different capture API, which is affected identically.
- The README describes the macOS first run: the Gatekeeper detour, the three permissions, the `+` fallback, the restart Screen Recording requires, and why an update costs the grants.

### Notes

- macOS builds are signed with a self-signed certificate rather than an Apple Developer ID, and are not notarized. What that costs, precisely:
  - **Gatekeeper blocks the first launch.** The user has to allow the app once through System Settings -> Privacy & Security. Only notarization removes this.
  - **The app does not appear in the Screen Recording list by itself.** macOS 15 and later require an Apple-issued Team ID before screen capture permission registers, and no certificate that Apple did not issue carries one. Adding the app with `+` works and permission then behaves normally.
  - **Permissions now survive updates.** This was the worst of it and is fixed: an update used to look like a different program to macOS, costing users all three grants. Verified on macOS 26 by installing two builds with different code hashes and the same certificate, with nothing re-granted in between.

  A Developer ID certificate and notarization close the remaining two, and switching to it will invalidate existing grants once, since the certificate is the identity.


## [0.14.0] - 2026-08-23

Two deliberate clicks a quarter of a second apart are no longer silently collapsed into one step, and every screenshot now records where its click marker was drawn.

### Fixed

- Clicks are suppressed only when they land at the *same cursor position* within a short window, instead of by time alone. The previous rule dropped any event within 300 ms of the last captured one with no knowledge of where the cursor was, so tabbing quickly through a form and clicking two different controls 250 ms apart discarded the second one. There was no log line and no counter; the step was simply missing from the finished document.
- Every suppressed event is now written to the log with its position and the interval since the previous capture. A rule that drops input invisibly is the defect; a narrower silent rule keeps it.
- Holding Enter no longer produces one screenshot per key repeat. Auto-repeat is now separated from deliberate input by the key release, which auto-repeat never emits, rather than by a clock, which cannot tell them apart at any usable width: Windows waits about 500 ms before the first repeat, so it arrives outside any window narrow enough to be safe. A few seconds of a held or stuck Enter previously pushed the screenshot count past the server's limit and the whole generation was refused.
- The log no longer discards its own oldest entries during a recording. It kept 40 KB and threw the overflow away, so a single Enter held for five seconds emitted enough auto-repeat lines to delete the first half-minute of the same recording. That made the suppression log, which exists so that dropped input can be audited afterwards, report that nothing had been suppressed in a recording where it had. Rotated files are now kept.
- A click that lands on the recorder's own window is now logged as ignored instead of vanishing. It has always been discarded on purpose, so that pressing "Start" or "Stop" never becomes a step, but it was the one remaining path that dropped input without leaving any trace, which is exactly what makes a missing step impossible to diagnose afterwards.
- A recording no longer sees each click once per recording started since the application launched. The input hook was installed again on every recording start and never removed, so the second recording of a session received every click twice, the third three times, and so on. The old time-only rule concealed this; the new suppression log is what surfaced it.

### Added

- Each screenshot's sidecar records `marker_box`, the click marker's bounding box in the *saved* image's pixels. The marker is painted into the image before it is written and the image is then downscaled by a factor that never left the machine, so nothing receiving these screenshots could previously locate it. The field is optional and absent for keypress steps, which draw no marker.

### Notes

- `marker_box` is written only by this version onward. Sidecars from earlier builds still parse and still align, and recordings made before this release simply carry no marker geometry.
- The suppression window is 250 ms and is bounded at 300 ms by design, so this rule can never drop an event the previous one kept.

## [0.13.1] - 2026-08-18

### Fixed

- A remembered pipeline choice that no longer exists on the server is now cleared from the stored settings, not only from the dropdown. The review screen reset the visible selection to "Standard" while the upload continued to send the old id (it is read from the store, not from the screen), so a pipeline that had been renamed, deleted, or unlabelled server-side produced a generation refused for a guide type the user could see they had not selected.

## [0.13.0] - 2026-08-18

Users can choose which kind of guide a recording should become, when the server offers a choice.

### Added

- Pipeline selector ("Art der Anleitung") on the review screen. The recorder fetches the catalogue from the server's `GET /pipelines` and sends the chosen `id` with the upload, so a new guide type appears in an already-installed build with no application update. The selection is remembered and preselected for the next recording, and the selected entry's description is shown as supporting text below the dropdown.
- The selector is available when generating from an existing recording folder as well, which has no record-time moment at which a choice could otherwise be expressed.

### Changed

- "Aus Ordner generieren" now opens the review screen instead of generating immediately. The flow is the same whether or not pipelines are configured: review the capture, choose a guide type if offered, then confirm.
- A refused upload now shows the message the server wrote for the user (for example "Die gewählte Anleitungsart ist derzeit nicht verfügbar...") instead of the raw JSON response body. The full body is still written to the log.

### Notes

- The selector renders only when the server offers two or more pipelines. Zero entries, one entry, and an unreachable endpoint all render nothing and never block recording, so installations without pipelines configured see no change at all.
- It is independent of `advanced_settings`: every organization may pick a guide type, and none gains the pipeline-version, model, or upload-target controls by doing so.

## [0.12.6] - 2026-06-20

Generate-from-folder is now available to all users, with a guard for invalid folders.

### Changed

- "Aus Ordner generieren" (generate from an existing recording folder) is now shown to all users in release builds; it was previously gated to dev builds (`import.meta.env.DEV`). It appears as a secondary button below the permanent "Aufnahme starten" CTA on the idle/done/error states and reuses the currently selected model/pipeline, upload, and persistence -- letting users regenerate an SOP from a previously captured recording without re-recording (for example, to retry with a different model). The primary "Aufnahme starten" CTA is unchanged.

### Added

- Invalid-folder guard for "Aus Ordner generieren": picking a folder that is not a recording session (no `screenshots/`) is rejected up front with a friendly message ("Dieser Ordner enthält keine Aufnahme...") via a cheap local check, instead of failing mid-pipeline after upload.

## [0.12.5] - 2026-06-01

Staging-target switching for advanced orgs, login-screen cleanup, and a small dependency security bump.

### Added

- "Konto erstellen" link on the login screen, next to "Passwort vergessen?", opens the webapp signup page (`/signup`) for the currently configured backend.
- Auto-logout on backend switch: changing `upload_target` in Settings now invalidates the current session (Supabase JWTs are bound to one backend) and bounces the user to the login screen instead of letting the next request 401 silently.

### Changed

- `upload_target` dropdown is now visible to orgs in `ADVANCED_SETTINGS_ORGS` regardless of build mode (was: dev builds only). The `Local` option remains dev-only since it points at localhost; advanced orgs running a release binary see only `Staging` and `Production`. Server-side gating is unchanged -- end users never see the dropdown.
- Release builds now honor `upload_target` for API and webapp URL resolution (dropped the `cfg!(debug_assertions)` gates in `commands::auth::get_api_base`, `commands::generate::run_generation_inner`, and `commands::settings::get_webapp_url`). Required so advanced-org users can actually reach Staging from a downloaded release binary; end users still can't see the dropdown.
- Login screen no longer renders the Settings gear -- pre-login backend choice is no longer needed since switches happen post-login via the dropdown.
- Staging webapp URL corrected from `https://app.staging.cogniclone.ai` to `https://staging.cogniclone.ai`.

### Security

- `openssl` 0.10.79 -> 0.10.80 (medium; potential out-of-bounds write in `CipherCtxRef::cipher_update_inplace` for AES-KW-PAD ciphers).
- `tar` 0.4.45 -> 0.4.46 (medium; PAX header desynchronization issue).

## [0.12.4] - 2026-05-31

Self-host enablement: runtime config overrides for backend URLs/updater, plus a staging upload target for dev builds.

### Added

- Runtime configuration overrides for backend URLs and the updater. Resolution precedence: environment variables (`COGNICLONE_API_URL`, `COGNICLONE_WEBAPP_URL`, `COGNICLONE_UPDATER_ENABLED`) > TOML file at `%APPDATA%\CogniClone\config.toml` (platform-equivalent on other OSes) > compile-time defaults. Enables self-host deployments where the backend runs on customer infrastructure and the GitHub-based updater may need to be disabled (airgapped networks). SaaS builds see no behavior change unless an override is set.
- Staging upload target in dev builds: Settings now offers `Local`/`Staging`/`Production` for `upload_target`, with `Staging` routed to `https://api.staging.cogniclone.ai` and `https://app.staging.cogniclone.ai`. Release builds remain pinned to production.

## [0.12.3] - 2026-05-11

Backend-driven generation options so server-side pipeline/model changes propagate to the recorder without a release.

### Added

- `GET /me/quota` now returns a `generation_settings` block (`pipeline_versions`, `models`, `default_model`) which Settings consumes to populate the pipeline-version and model dropdowns. The recorder falls back to `pipeline_versions: [1, 2]` and `azure/gpt-4.1` when the field is absent so older servers keep working.

## [0.12.2] - 2026-05-09

Dependency security release for user-facing runtime alerts.

### Security

- Updated Tauri from 2.10.3 to 2.11.1 to fix the local-origin IPC origin confusion advisory
- Updated `openssl` from 0.10.78 to 0.10.79 to fix OCSP URL UTF-8 undefined behavior and AES key-wrap-with-padding heap overflow advisories
- Updated `imageproc` from 0.25.0 to 0.25.1 to fix out-of-bounds read and fragile bounds-check advisories
- Updated `postcss` from 8.5.8 to 8.5.10 to fix CSS stringification XSS advisory in development dependencies
- Removed the vulnerable transitive `rand` 0.7.3 path by updating the Tauri dependency stack

## [0.12.1] - 2026-05-04

Legal-page links repointed to the public marketing site after consolidating Impressum, Datenschutzerklärung, and Nutzungsbedingungen on `cogniclone.ai`.

### Changed

- PII confirmation dialog links now open `https://cogniclone.ai/impressum/`, `/datenschutzerklaerung/`, and `/nutzungsbedingungen/` instead of the deprecated `app.cogniclone.ai/{legal,privacy,terms}` paths
- "Rechtliches" label renamed to "Impressum" to match the new public page slug

## [0.12.0] - 2026-04-28

Webapp domain switch to app.cogniclone.ai, per-screenshot sidecar metadata, and security dependency bumps.

### Added

- Per-screenshot sidecar JSON: each capture writes a `step_NN.json` next to `step_NN.png` with `order`, `timestamp_seconds` (since recording start), `click_x/y`, and `trigger`. The upload aggregates these into `metadata.steps[]` so the server can align narration to screenshots without pre-slicing
- Crash-resilient capture: each step self-describes after the PNG write succeeds, so aborted recordings always leave matched (png, json) pairs

### Changed

- `WEBAPP_URL_PROD` now points to `https://app.cogniclone.ai`
- Legal/privacy/terms links in the PII confirmation dialog open `app.cogniclone.ai/{legal,privacy,terms}`

### Removed

- Parallel `Mutex<Vec<f64>>` + `pending.json` screenshot_timestamps from an earlier iteration (superseded by sidecar JSON)
- Unused `CapturedStep` struct that was never populated

### Security

- `openssl` 0.10.76 -> 0.10.78 (4 alerts: 3 high, 1 low; buffer-overflow + bounds-assertion fixes in derive/aes-key-wrap/digest_final/PSK)
- `rustls-webpki` 0.103.10 -> 0.103.13 (1 high DoS via CRL; 2 low name-constraint issues)
- `rand` 0.8.5 -> 0.8.6 (low; custom-logger unsoundness)
- Note: `rand@0.7.3` alert remains, pulled in by `tauri-utils` via `phf_generator 0.8.0` at build time only; no exploit path and no upstream fix without Tauri changes

## [0.11.0] - 2026-04-15

Server-side PDF generation, model selection, and dependency updates.

### Added

- Server PDF download: the recorder fetches the branded PDF from the server via signed URL, with local genpdf as fallback
- Model selection setting: choose between GPT-4.1 and Claude Sonnet 4.6 in the settings page (advanced orgs only)
- Pipeline version setting: choose between V1 (single-call) and V2 (analysis + generation) pipelines (advanced orgs only)
- Generation model is sent as metadata to the server for per-request model override
- Org feature flags: model and pipeline dropdowns are gated by `ADVANCED_SETTINGS_ORGS` server env var, fetched via `/quota` response

### Changed

- PDF output now uses the server-generated branded PDF (CogniClone teal palette, Orbitron/Manrope fonts, prereq bar, result section) instead of the basic local genpdf output
- SSE result payload now includes `pdf_url` field for server PDF download

### Fixed

- Updated rand 0.9.2 to 0.9.4 to fix Dependabot security alert (RUSTSEC advisory)

### Dependencies

- Vite upgraded from v7 to v8 (fixes 3 security vulnerabilities)
- @vitejs/plugin-react upgraded to v6 for Vite 8 compatibility

## [0.10.0] - 2026-04-12

Full visual rebrand from SOP Sorcery to CogniClone AI.

### Changed

- Product name, window title, and all user-visible strings renamed from "SOP Sorcery" / "aprodo" to "CogniClone"
- Color palette replaced: teal primary (#2CB5C0) with warm charcoal surfaces (#1E2328), replacing the old blue (#80aeff) on black (#0e0e0e)
- Body font switched from Segoe UI to Manrope; Orbitron used for the logo wordmark
- App icons regenerated from the new CogniClone logo (Logo Rund.svg)
- Login screen shows CogniClone AI logo and wordmark
- Update banner gradient uses teal instead of blue
- CTA glow and stop-pulse animations updated to match new palette
- Window height increased from 380 to 440 to accommodate the logo
- App identifier changed to com.cogniclone.recorder
- Keyring service name changed to "cogniclone"
- Legacy migration now checks for "CogniClone Workflows" folder

### Added

- Keyring migration: automatically transfers stored credentials from old "sop-sorcery" keyring to "cogniclone" on launch
- Orbitron and Manrope fonts bundled locally via @fontsource (no external Google Fonts dependency)

## [0.9.0] - 2026-04-11

Recorder UX overhaul for the demo build: quota visibility, review-before-generate flow, and a richer compact recording bar.

### Added

- Quota chip on main screen showing "N / limit Anleitungen", with warning colors when the remaining quota is low
- Pre-emptive quota check: when the user is already at their limit, the rate-limit modal opens instead of touching the microphone
- Rate limit modal with German quota-exhausted messaging (count/limit, upgrade hint, dismiss)
- Review screen: after `Stop`, the user inspects the captured screenshots before committing to a generation; confirm runs the pipeline, cancel discards
- Retry-from-disk button on `idle`/`error` screens: re-runs generation against the preserved session directory without losing captured steps
- Compact recording bar telemetry: live capture counter, elapsed time, and VU-style audio level meter fed by a new `recording:audio_level` event
- Undo-last-screenshot button in the compact bar, backed by a new `delete_last_screenshot` Tauri command that also emits `recording:step_deleted`
- Microphone permission warning chip shown on launch when the OS has denied mic access
- New React hooks: `useQuota`, `useCaptureCount`, `useElapsedTime`, `useAudioLevel`
- New Tauri commands: `get_quota`, `get_microphone_permission_state`, `list_session_screenshots`, `read_screenshot_bytes`, `delete_last_screenshot`
- Server: `GET /quota` endpoint returning `{count, limit, remaining}` (new `server/routes_quota.py`), plus transcript upload (`transcript.md`) alongside the generated guide

### Fixed

- Stop-recording race: `stop_recording` now waits up to 15s for in-flight screenshot captures to finish writing before returning, so the review screen opens onto a stable filesystem state
- Retry flow getting stuck busy: `handleRetry` now delegates to `confirmGeneration()`, which correctly walks the post-generation state machine into `done`
- Infinite `/quota` refresh loop caused by `useQuota`'s object identity changing every render; quota refresh is now driven by a stable `refreshQuotaRef` so effects only fire on real state transitions
- Review-screen thumbnails broken under Tauri v2: now loaded via `read_screenshot_bytes` + Blob URLs instead of the unconfigured asset protocol
- English "stopping" sentinel leaking to the UI during stop-to-review transitions; localization is now applied at the `useRecorder` source

### Changed

- Compact recording bar widened to 240x34 to fit counter, undo, and VU meter
- Cancel button in the compact bar recolored to error red; Stop uses the primary color
- All new status strings localized in German (`status.stopping` as "Aufnahmen werden verarbeitet...", `status.retry_from_disk`, `status.undo_last`, `review.*`, `quota.*`, `mic.permission_denied`)
- Dev-only asyncio exception filter on Windows to suppress noisy transient `OSError`s (WinError 64/121/1236/10053/10054) from client-abort disconnects; gated on `ENVIRONMENT != "production"` so Linux/prod behavior is unchanged

## [0.8.4] - 2026-04-02

### Changed

- Suppress noisy third-party log output: keyring, tao, tauri_plugin_updater, and reqwest::retry set to Warn level
- Global log default set to Info (was unset, allowing DEBUG from all crates)
- reqwest::connect kept at Info in dev builds for debugging, suppressed to Warn in release builds

### Added

- Slack release notification step in CI via CCBot webhook

## [0.8.3] - 2026-03-31

### Changed

- PII blocked modal simplified: removed legal disclaimer footer, friendlier tone, points to settings
- PII toggle in settings now shows confirmation modal with full disclaimer before disabling
- Legal links (Rechtliches, Datenschutz, AGB) moved to settings confirmation modal
- Default logs directory derived from productName in tauri.conf.json instead of hardcoded path
- Default workflows directory derived from productName in tauri.conf.json
- Settings defaults now persisted to store on first launch (no longer recomputed each time)

### Added

- PII disabled chip on main screen when safety check is off, links to settings
- Legacy migration: preserves existing "Wizimate Workflows" folder for upgrading users
- SettingsPage test suite (7 tests for confirmation modal flow)
- RecorderScreen tests for PII disabled chip (5 tests)
- i18n required keys coverage for all PII-related strings

## [0.8.2] - 2026-03-31

### Added

- Job polling recovery: when SSE stream disconnects mid-generation, the client polls the server for the result instead of failing
- `jobs.rs` network module for server-side job status polling

### Fixed

- SSE disconnect during generation no longer loses the result

## [0.8.1] - 2026-03-31

### Fixed

- Token expiration on consecutive recordings: access token is now refreshed before each upload
- 401 errors during upload trigger a second token refresh and retry
- Permanently expired sessions emit `auth:session_expired` event, forcing re-login with a clear message

### Added

- `useAuth` listener for `auth:session_expired` backend event
- Tests for session expiry handling in `useAuth`

## [0.8.0] - 2026-03-30

### Added

- Cancel button in compact recording bar with native OS confirmation dialog
- Draggable compact recording bar with custom drag region and move icon
- Auto-position compact bar to bottom-right corner above taskbar on recording start
- PII blocked modal overlay showing which steps and entity types were detected
- Copy button in PII modal to save findings to clipboard before dismissing
- Legal disclaimer with links to privacy policy, terms, and legal pages in PII modal
- Rust `get_work_area()` command for accurate taskbar-aware window positioning
- German translations for PII entity types (IBAN, Steuer-ID, Sozialversicherungsnr., etc.)

### Fixed

- Error events now properly transition recorder to error state with red styling
- PII blocked events are handled via dedicated `pii_blocked` status instead of being silently dropped
- Done message now shows "Gespeichert und hochgeladen" instead of generic text

### Changed

- Compact recording bar redesigned: Cancel | drag handle | Stop layout (200x32)
- Success message uses `done_uploaded` translation (server always has the result on success)

### Removed

- Dead translation keys: `status.done`, `status.pending_found`, and 15 other unused keys from previous versions

## [0.7.6] - 2026-03-28

### Fixed

- Fix heartbeat events showing as raw JSON in the UI instead of being silently consumed
- Add job_id tracking to SSE result and error events for generation durability

## [0.7.5] - 2026-03-27

### Added

- Skip PII check toggle in settings to bypass the server-side PII guardrail per-request
- SSE stream debug logging and CRLF normalization for cross-platform reliability

## [0.7.4] - 2026-03-25

### Added

- Dismissible update banner when a new version is available

## [0.7.3] - 2026-03-24

### Fixed

- Force production URLs in release builds, allow local server only in dev mode

## [0.7.2] - 2026-03-24

### Added

- Deploy step in CI to upload signed installer to server
- Read app version from tauri.conf.json instead of hardcoding

### Fixed

- SSH key handling in release deploy step

## [0.7.1] - 2026-03-21

### Fixed

- CI permissions for release workflow
- Updated dependencies

## [0.7.0] - 2026-03-21

Full rewrite of the SOP Recorder from Python/CustomTkinter to Tauri v2 (Rust + React + TypeScript).

### Added

- Tauri v2 desktop app replacing the Python/CustomTkinter recorder
- Screen capture on mouse click via `xcap` with red dot + cursor overlay
- Audio recording via `cpal` with linear-interpolation resampling (48kHz to 16kHz)
- Global input hooks via `rdev` (mouse click + Enter key, 300ms debounce)
- Server upload with multipart form-data and SSE progress streaming
- Local PDF generation via `genpdf` with Segoe UI / Calibri / Arial fonts
- Local markdown output (`guide.md`)
- Supabase authentication via FastAPI server proxy
- Secure token storage in Windows Credential Manager via `keyring` crate
- Settings in a separate window (hide-from-screenshots, output/logs directory, folder picker)
- Crash recovery with `pending.json` and retry flow
- Auto-updater via GitHub Releases (`tauri-plugin-updater`)
- System tray with context menu (Show/Hide, Start/Stop, Settings, Quit)
- German UI with proper umlauts (i18n)
- DPI awareness for multi-monitor setups
- `SetWindowDisplayAffinity` to hide recorder from screenshots
- Per-user NSIS installer (no admin rights required)
- CI/CD with GitHub Actions (frontend + Rust tests, build, release)
- 32 frontend tests (vitest) and 11 Rust unit tests

### Changed

- Installer size reduced from ~150MB (PyInstaller) to ~10MB (Tauri NSIS)
- Screenshots now saved in `screenshots/` subdirectory (was flat in output dir)
- Screenshots saved as RGB PNGs (was RGBA, which Azure OpenAI rejected)

[Unreleased]: https://github.com/wizi89/sop-recorder/compare/v0.16.0...HEAD
[0.16.0]: https://github.com/wizi89/sop-recorder/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/wizi89/sop-recorder/compare/v0.14.0...v0.15.0
[0.13.1]: https://github.com/wizi89/sop-recorder/compare/v0.13.0...v0.13.1
[0.13.0]: https://github.com/wizi89/sop-recorder/compare/v0.12.6...v0.13.0
[0.10.0]: https://github.com/wizi89/sop-recorder/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/wizi89/sop-recorder/compare/v0.8.4...v0.9.0
[0.8.4]: https://github.com/wizi89/sop-recorder/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/wizi89/sop-recorder/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/wizi89/sop-recorder/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/wizi89/sop-recorder/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/wizi89/sop-recorder/compare/v0.7.6...v0.8.0
[0.7.6]: https://github.com/wizi89/sop-recorder/compare/v0.7.5...v0.7.6
[0.7.5]: https://github.com/wizi89/sop-recorder/compare/v0.7.4...v0.7.5
[0.7.4]: https://github.com/wizi89/sop-recorder/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/wizi89/sop-recorder/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/wizi89/sop-recorder/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/wizi89/sop-recorder/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/wizi89/sop-recorder/releases/tag/v0.7.0
