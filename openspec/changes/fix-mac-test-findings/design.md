## Context

See `proposal.md` — Why. This section records only the current implementation facts the approach depends on. Line references are against v0.16.0 (`c8e7efa`).

- **Capture pipeline.** `input_hooks::dispatch` calls a session callback on the OS hook thread. `commands/recording.rs:191` spawns one `std::thread` per event, which calls `screenshot::capture_and_save`. `capture_full_screen` (`screenshot.rs:71`) captures every `xcap::Monitor`, measures a shared canvas scale, composites them, and returns the canvas with a `VirtualScreen { origin, scale }`. `capture_and_save` draws the marker, then caps the image at 1920×1080 and scales `MarkerBox` by the same factor.
- **Upload.** `run_generation_inner` (`generate.rs:90`) builds `screenshot_paths` with a `loop` that increments `step_num` and `break`s on the first non-existent path. `step_meta::read_all` is only forwarded when its length equals `screenshot_paths.len()`.
- **Settings.** `commands/settings.rs` stores everything in a `tauri-plugin-store` file `settings.json`. `get_settings` (`:120`) ends with `keyring_load("openai-key")`. `save_settings` (`:181`) mutates the store and returns; the plugin's default `auto_save` is a 100 ms debounce (`tauri-plugin-store` 2.4.3, `store.rs:68`). `AppSettings::defaults` (`:79`) derives `logs_dir` from `app.path().app_local_data_dir()`, while `tauri-plugin-log` 2.8.0 defaults to `TargetKind::LogDir` — `app_log_dir()`.
- **Settings UI.** `SettingsPage.tsx:77` does `getSettings().then(setSettings)`, replacing the whole form state. `handleSave` (`:104`) awaits `saveSettings`, then closes the window; the catch only logs.
- **Constraints.** The click marker's `MarkerBox` travels to the server, which blanks it out of both frames before perceptual comparison — its geometry is a contract. `xcap` is the only capture backend. macOS AppKit calls must stay on the main thread; the capture path already avoids them.

## Goals / Non-Goals

**Goals:**

- Make every data-loss path in the list either impossible or visible. Where a failure can still happen (a capture that genuinely fails), it is counted and shown.
- Keep changes decomposable into pure functions that can be tested without a screen, a keychain, a window, or a network.
- Preserve the server contracts: `MarkerBox` geometry, `step_NN.json` shape, upload multipart fields.

**Non-Goals:**

- A pre-recording display picker. Per-click monitor selection removes the measured quality loss without new UI; the picker stays in the backlog.
- Making `logs_dir` configurable. The log target is fixed when the plugin is built, before an `AppHandle` exists; honouring a stored path would mean re-initialising logging after startup.
- Reworking step granularity or the step/transcript prompt — see the proposal's non-goals.

## Decisions

### Enumerate screenshots by reading the directory

Replace the counting loop with a pure `collect_step_screenshots(dir) -> Result<Vec<(u32, PathBuf)>, String>`: read the directory, match `step_(\d+).png`, parse the number, sort numerically, return. Gaps are logged with the missing numbers.

*Alternative considered:* keep the counting loop but continue past a gap up to a bound. Rejected — it needs an arbitrary bound and still can't tell "step 7 failed" from "the recording ended at 6". Reading the directory answers both.

**Correction, made during implementation.** `step_meta::read_all` had the *same* count-and-stop defect, with a test pinning it (`read_all_stops_at_first_gap`). Fixing only `generate.rs` would have shipped all 20 screenshots with zero per-step timestamps, because the sidecar count would still stop at 1 and fail the length check — turning finding A into finding G for exactly the recordings that hit it. Both scans are now directory reads. The earlier claim in this document that fixing A "closes that gap" for G was wrong as written: it needs this second fix to be true.

**Verified against the backend** (`sop-sorcery`, `server/routes_generate.py:260-280`): `metadata.steps` is paired to the screenshots **by position**, gated only on `len(raw_steps_meta) == len(screenshots)`, and `order` is never read. Screenshots are ordered by `sorted(form.keys())`. Sparse-but-ascending step numbers therefore pair correctly with no server change — renumbering to a contiguous 1..N at upload time was considered and rejected as unnecessary.

Numeric sort matters: lexicographic ordering puts `step_10` before `step_09`. The current `{:02}` format masks this below 100 steps, so the sort is on the parsed integer, not the filename.

### Bound capture concurrency with a semaphore, keep the thread-per-event shape

A `std::sync::Arc<Semaphore>`-equivalent (a `Mutex<usize>` + `Condvar`, or `tokio::sync::Semaphore` with `blocking_acquire`) with 2 permits, acquired inside the spawned thread before capturing. The step number is still assigned on the hook thread, before the spawn, so ordering is unaffected by queueing.

*Alternative considered:* a single worker thread with a channel. Rejected — it serialises captures on a slow machine where two can usefully overlap (one capturing while the other encodes PNG), and the existing `in_flight` counter and `stop_recording` wait already model "work outstanding" correctly for a queue.

Permit count 2 rather than 1: capture and PNG encode are the two costs, and they pipeline. Higher counts reintroduce the memory pressure this fixes.

### Capture one monitor, chosen by a pure function

`monitor_for_click(point: Option<(i32, i32)>, bounds: &[(i32, i32, u32, u32)]) -> usize` returns the index of the containing monitor, falling back to the primary (index of the monitor at the smallest origin, matching `xcap`'s ordering guarantee being absent — resolved explicitly by asking `Monitor::is_primary`).

`capture_full_screen` is kept but becomes `capture_monitor(index) -> (RgbaImage, VirtualScreen)` where `VirtualScreen.origin` is that monitor's origin and `scale` is measured from that monitor's own geometry-to-capture ratio. Everything downstream — `render_click_overlay`, `marker_box_at`, the 1920×1080 cap — is unchanged, because it was already written against `VirtualScreen` rather than against "the whole desktop".

*Alternative considered:* keep the composite and raise the size cap. Rejected — the cap exists for the 4 MB Azure OpenAI image limit, and a composite is the wrong image anyway: it shows the user a screen they weren't looking at.

*Consequence:* when the cursor's monitor cannot be determined for an Enter-triggered step, the step captures the primary display, which may not be the one being worked on. This is a behaviour change from "always shows everything" to "usually shows the right one, occasionally the wrong one". It is the right trade: the composite was legible on neither.

### Draw the marker as a ring

`imageproc`'s `draw_filled_circle_mut` writes pixels; it does not blend, so `Rgba([255,0,0,179])` is opaque red. Replace with `draw_hollow_circle_mut` at two or three radii (a 3 px stroke at scale 1, scaled with the canvas) so the stroke is solid at Retina scales too.

`marker_box_at` is not touched: the ring's outer radius equals the old disc radius, so the reported bounds stay byte-identical and the server's masking keeps working. This is the reason the box is computed from geometry rather than from the drawing.

*Alternative considered:* real alpha blending via `imageproc::drawing::Blend`. Rejected — a 70 %-opaque red wash still degrades OCR of the text under it, which is the whole complaint.

**Revised during implementation: the cursor arrow is removed too.** This document said "keep the white arrow"; the first test showed why that cannot stand. `ARROW_OFFSETS[0]` is `(0, 0)`, so the arrow's tip is drawn *on* the click point and its filled body covers 15×25 px of the control down and right of it. The ring alone therefore does not fix the finding — it removes the disc and leaves the arrow sitting on the button label. It is also redundant (the ring already localises the click), misleading (the same arrow glyph is drawn whatever the real cursor was, so a text field gets an arrow it never had), and the least survivable part of the image through the 1920×1080 downscale. Removed; `ARROW_OFFSETS` is retained solely because `marker_box_at` is a server contract. The reported box is now larger than the drawing, which costs a marginally blinder near-duplicate comparison — the same region was masked before, when the arrow occupied it.

A white hairline is drawn on each edge of the red stroke, both inside `radius`. A bare red ring disappears against red or dark application chrome, and keeping the hairlines inside the radius means the outer edge, and so the reported box, does not move.

### Split credential access out of `get_settings`

`get_settings` stops returning `api_key`; a new `has_api_key() -> bool` command answers the only question the UI asks. `save_settings` keeps accepting an optional `api_key` for the write path.

This is a breaking change to the `AppSettings` payload shared with TypeScript. The field is dropped from the interface in the same change; `src/test/settings.test.tsx` and `errorReportFlow.test.tsx` fixtures are updated with it.

*Alternative considered:* keep the field but read the keychain lazily in a background task. Rejected — it leaves the same race in a narrower window, and the UI never needs the key's value.

### Flush the store explicitly

`save_settings` calls `store.save()` before returning, and propagates the error. The 100 ms debounce stays as a backstop for other writers (`pipelines.rs`, `auth.rs`).

### Gate the settings form on a `loaded` flag

`SettingsPage` gets `loaded: boolean`, set once when the first `getSettings` resolves. Controls carry `disabled={!loaded}`. The quota effect keeps its `setSettings(s => ...)` functional update — it only reconciles `generation_model` against the server's list and must not become a wholesale replace.

`handleSave` gains a `saveError` state; the window closes only on success.

### Derive `logs_dir` from the log plugin's own resolver

`AppSettings::defaults` uses `app.path().app_log_dir()`. `AppSettings::initialize` additionally overwrites a stored `logs_dir` that differs, so a settings file from an older build is corrected at startup rather than only when the user saves.

The UI field becomes `readOnly` with a reveal button using the already-present `@tauri-apps/plugin-opener` `revealItemInDir`, which `App.tsx` already uses for the output folder.

### Surface failed captures

`RecordingSession` gains `failed_captures: Arc<AtomicU32>`, incremented in the capture thread's `Err` arm alongside the existing `log::error!`. `stop_recording` returns it, an event `recording:step_failed` carries the running count during recording, and `ReviewScreen` renders a warning line when it is non-zero. The count also goes into the error-report settings context so a report from that session carries it.

## Risks / Trade-offs

- **Per-monitor capture changes what guides look like for existing multi-monitor users** → The change is strictly toward the user's actual working screen and is what single-monitor users already get. Called out in the changelog; verified in the 2026-09-21 session with the tester who raised it.
- **A wrong primary-display fallback for key-triggered steps** → Only reachable when the cursor position is unavailable, which is already rare enough that the input hook treats it as exceptional. Logged when it happens, so the frequency is measurable rather than assumed.
- **Semaphore of 2 could slow a machine that was coping** → The existing `stop_recording` wait already absorbs a backlog with a 15 s cap, and the queue only forms during bursts faster than capture. Capture duration is logged per step, so if the cap is wrong the data says so.
- **Dropping `api_key` from `AppSettings` touches the shared TS interface** → Compile-time failure in TypeScript and Rust, not a silent one; the test fixtures are the only other consumers.
- **`app_log_dir()` and the plugin's `LogDir` could diverge in a future plugin version** → The startup correction runs every launch, so a divergence self-corrects in the file; a unit test asserts the two agree.
- **The gap-tolerant enumeration will now upload screenshots from a recording the user thought was shorter** → This is the intended behaviour, and the failed-capture notice tells the user what happened. Without it, the data was being discarded instead.

## Migration Plan

No data migration. Two stored-state effects, both handled at startup and both idempotent:

1. `logs_dir` in `settings.json` is rewritten if it disagrees with the real log directory.
2. Nothing else in `settings.json` changes shape; `api_key` was never stored there (it lives in the keyring).

Rollback is a straight revert: no persisted format changes, and the server contracts (`MarkerBox`, `step_NN.json`, multipart fields) are unchanged by design, so an older client and a newer one interoperate with the same backend.

## Open Questions

- Whether the server should be told which display a step was captured from. Not needed for any current behaviour, and adding a field later is backward compatible — deferred rather than guessed at.
