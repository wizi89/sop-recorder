# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/wizi89/sop-recorder/compare/v0.7.5...HEAD
[0.7.5]: https://github.com/wizi89/sop-recorder/compare/v0.7.4...v0.7.5
[0.7.4]: https://github.com/wizi89/sop-recorder/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/wizi89/sop-recorder/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/wizi89/sop-recorder/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/wizi89/sop-recorder/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/wizi89/sop-recorder/releases/tag/v0.7.0
