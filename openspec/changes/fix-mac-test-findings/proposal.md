## Why

An external tester ran four end-to-end recordings of the macOS build (v0.16.0) on 2026-09-03. Two runs produced a usable guide; two produced a guide containing a single step despite ~20 deliberate clicks, and the tester was left with a false theory about the cause because the app's own settings file disagreed with the app's behaviour. Tracing every observation back to source showed six independent defects, four of which destroy recorded work or user input silently — the app reports success while dropping data. The tester's leading hypothesis (that `skip_pii_check = true` suppresses screenshots) is disproved by the code and must not be carried into the follow-up session on 2026-09-21.

## What Changes

- **Screenshots after a failed capture are no longer discarded.** `run_generation` enumerates `step_NN.png` by counting upward and stops at the first gap, so one failed capture silently removes every later screenshot from the upload. Enumeration becomes gap-tolerant, and a capture failure becomes a counted, user-visible outcome instead of a log line.
- **Screenshot capture concurrency is bounded.** Every click spawns an unbounded thread that allocates a full-virtual-desktop RGBA canvas plus resize copies (~66 MB per in-flight capture on two 4K displays). Captures are queued behind a small semaphore.
- **Captures cover one monitor, not the whole virtual desktop.** The composited canvas is hard-capped to 1920×1080 before upload, so each of two 4K displays arrives at 960×540 — measurably worse OCR (the tester's guide named the wrong application). Capture targets the monitor the click landed on.
- **The settings window stops discarding user edits.** The form replaces its entire state when the asynchronous load resolves, so any toggle changed before then is reverted without a visible cue. The load is also slowed by a macOS Keychain read that widens the window to seconds after a reinstall.
- **Saving settings becomes durable and reports failure.** `save_settings` relies on the store plugin's 100 ms debounce and never flushes; the window closes immediately after, and a failed save reaches only `console.error`.
- **`logs_dir` reports the directory logs are actually written to.** The default is derived from `app_local_data_dir()` while `tauri-plugin-log` writes to `app_log_dir()`; these coincide on Windows and differ on macOS. The field also becomes read-only, because nothing reads it.
- **The click marker stops covering the element that was clicked.** `draw_filled_circle_mut` writes pixels rather than blending, so the "semi-transparent" red disc is fully opaque over the click target. It becomes a ring.
- **Processing shows that it is still working**, and a recovered SSE reconnect is no longer reported as an interruption.

Non-goals, recorded so the follow-up session does not relitigate them:

- Per-step transcript excerpts (spoken content missing from guides) are a server-side prompt change. This change only guarantees the client stops silently dropping the step/audio alignment it already sends.
- macOS code signing and notarization are release-engineering work, tracked separately.
- Product-shape requests from the same session (step granularity, per-step carousel, removing click-undo, cropping) are backlog, not this change.

## Capabilities

### New Capabilities

- `step-capture`: turning an input event into a saved screenshot — which monitor is captured, how the click is marked, how many captures may run at once, and what happens when one fails.
- `guide-generation`: assembling a recorded folder into an upload — screenshot enumeration, per-step metadata alignment, and what the user is told while it runs.
- `app-settings`: loading, editing, and persisting user settings, including the durability of a save and the reported log location.

### Modified Capabilities

None — `openspec/specs/` is empty; this is the first change to declare specs for these areas.

## Impact

Affected code:

- `src-tauri/src/capture/screenshot.rs` — monitor selection, click marker rendering, resize budget.
- `src-tauri/src/commands/recording.rs` — capture concurrency, capture-failure accounting, `recording:step_failed` event.
- `src-tauri/src/commands/generate.rs` — screenshot enumeration, sidecar alignment logging.
- `src-tauri/src/commands/settings.rs` — `logs_dir` default, explicit store flush, keychain read removed from the load path, new `has_api_key` command.
- `src/components/SettingsPage.tsx` — gated load, save error handling, read-only logs field with a reveal button.
- `src/components/ReviewScreen.tsx`, `src/components/StatusBar.tsx`, `src/hooks/useSSE.ts` — failed-step notice, elapsed-time and heartbeat display, reconnect wording.
- `src/lib/tauri.ts`, `src/i18n/de.ts` — command bindings and copy for the above.

Compatibility:

- `MarkerBox` geometry is unchanged, so the server-side marker masking contract is unaffected.
- A stored `logs_dir` written by an older build is overwritten at startup. No other stored settings change shape.
- Screenshots become per-monitor images rather than virtual-desktop composites; `step_NN.json` click coordinates remain in the same image space as the PNG they accompany.

Dependencies: none added.
