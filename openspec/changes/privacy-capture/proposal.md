# Proposal

## Why

Every step today captures the whole monitor the click landed on: taskbar, tray, notifications, every other open window, and any password field on screen. Privacy by default is a core promise of the recorder. Capturing only what the step is about, and never a password field unless the user chooses so, is the largest privacy gain for the least work.

## What Changes

- New user setting **capture mode**: `active_window` (default) or `monitor`. Active window keeps only the area of the foreground window and its root owner, so a dialog keeps its parent app as context; the gap between a dialog and a distant owner is filled. Windows drawn above that area (notifications, always-on-top windows) stay visible where they overlap it. Monitor is today's behaviour.
- New user setting **mask passwords**, on by default. On Windows, every password field (UI Automation `IsPassword`) visible in the saved area, in any window, is filled opaque before the PNG is written to disk. The user can switch it off; the setting then says plainly that password fields are captured. No confirmation dialog.
- The check does not delay screenshots: pixels are taken at the input event, the check runs afterwards on the in-memory image. Only when six earlier steps are still being processed does a step wait, as captures queue today, and the wait is logged.
- Every case where the check may have missed a field (timeout, refusal, a window never checked before, a field that vanished because the input submitted the form, hanging checks) flags the step. The review screen marks flagged steps, steps that fell back to the full display, and steps with no record; Generate needs one confirmation when any are marked.
- Each step's sidecar records how it was captured and the check outcome. The upload metadata gains counts of both, derived from the sidecars. The server logs them and changes no behaviour.
- macOS keeps monitor capture and no password masking in this change.
- **BREAKING (spec)**: `step-capture`'s "Capture is scoped to the screen the action happened on" now holds only for the monitor mode.

Out of scope: a "chosen window" mode, org-level enforcement of either setting, audio, macOS window capture and `AXSecureTextField`, blacking out by the user (change `review-redaction`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `step-capture`: capture scope depends on the capture mode; password fields are masked before the file is written; the check never delays a screenshot; uncertain steps are flagged and must be acknowledged on the review screen.
- `guide-generation`: the upload carries per-recording counts of capture scope and password check outcome.

## Impact

- `src-tauri/src/capture/screenshot.rs`, `commands/recording.rs`, `capture/permits.rs`: two capture phases, buffer slots held across both, area computation, masking before save.
- New Windows-only module: geometry snapshot (DWM, z-order), UIA query workers with budget and cap, warm-up via `SetWinEventHook`, per-window cache. `windows` crate gains features (final list from the spike).
- `src-tauri/src/output/step_meta.rs`: `capture_scope`, `password_check`.
- `src-tauri/src/commands/settings.rs`, `SettingsPage.tsx`, `i18n/de.ts`: two settings under "Datenschutz bei der Aufnahme".
- New Tauri command for per-step capture records; `ReviewScreen.tsx`: marks and acknowledgement.
- `src-tauri/src/network/upload.rs`: two metadata objects.
- Server: a new log line and trace metadata. Old servers ignore unknown metadata keys, so the recorder can ship first.
