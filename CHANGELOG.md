# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/wizi89/sop-recorder/compare/v0.8.4...HEAD
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
