# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/wizi89/sop-recorder/compare/v0.13.0...HEAD
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
