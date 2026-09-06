## 1. Diagnostics first

These land before anything else, so the 2026-09-21 session with the tester produces measurements rather than another hypothesis.

- [x] 1.1 Log monitor count, canvas dimensions and elapsed milliseconds for every capture in `screenshot::capture_and_save`; verify by running a recording and confirming one line per step in the log file.
- [x] 1.2 Log the resolved screenshot set at the start of `run_generation_inner` (count, first and last step number, any gaps); verify by generating from a folder with a manually deleted `step_02.png` and reading the line back.

## 2. Screenshot enumeration (spec: guide-generation)

- [x] 2.1 Add `collect_step_screenshots(dir) -> Result<Vec<(u32, PathBuf)>, String>` in `commands/generate.rs`: read the directory, match `step_<n>.png`, parse and sort numerically, log gaps. Verify with a Rust unit test over a tempdir containing `step_01`, `step_03`, `step_04` returning all three in order.
- [x] 2.2 Add unit tests for the remaining enumeration scenarios: empty directory returns `Err`, `step_10` sorts after `step_09`, non-matching filenames are ignored. Verify `cargo test` passes.
- [x] 2.3 Replace the counting loop at `generate.rs:90` with the new function; verify the existing generation path still uploads a gapless recording unchanged.
- [x] 2.4 Make `step_meta::read_all` gap-tolerant too (it had the same count-and-stop defect), extract `steps_for_upload` so the drop decision is testable, and log both counts with "alignment dropped". Verified by unit tests on the decision plus an end-to-end test that a recording with a failed capture keeps screenshot/sidecar order and length in step.

## 3. Capture failure accounting (spec: step-capture)

- [x] 3.1 Add `failed_captures: Arc<AtomicU32>` to `RecordingSession`, increment it in the `Err` arm of the capture thread in `commands/recording.rs`, and include it in the value `stop_recording` returns. The outcome handling is extracted as `note_capture_outcome` so it is testable without an `AppHandle`, a screen or a live session; 5 Rust tests cover counting, accumulation, a success leaving the count alone, keypress metadata, and concurrent failures from the capture threads.
- [x] 3.2 Emit `recording:step_failed` with the running failure count when a capture fails. **Verified against a real failure on 2026-09-06**: `screenshots/` was made unwritable mid-recording, and steps 2-4 failed with `Failed to save screenshot: Permission denied (os error 13)`, counting `1 failed so far`, `2`, `3`, then `Recording stopped with 3 failed capture(s)`. The folder was left holding `step_01` and `step_05`..`step_10` -- on the previous code that recording would have generated a one-step guide and discarded six captured steps in silence.
- [x] 3.3 Surface the count in `ReviewScreen` as "N of M steps could not be captured" and add the German copy to `src/i18n/de.ts`. Verified by TS tests for the notice rendering (2 failures shown, 0 and absent both silent) and by two `useRecorder` tests covering the joint: `stop_recording`'s `failed_captures` reaching the state the review screen reads.
- [x] 3.4 **Done by other means, and the original is not safely shippable.** Two blockers: `publish_error_report_context` takes `&AppSettings` and only runs on save/startup, so it cannot carry live recording state; and the server's report model sets `model_config = {"extra": "forbid"}` (`sop-sorcery/server/routes_client_reports.py:84`), so adding any field to `ErrorReport` or `ReportSettings` would 422 **every** report from the new client against the deployed server. The need is met instead by 3.1's `log::error!`, which names the step, the running failure count and the cause, and lands in the ring buffer that fills `log_tail` — an accepted schema field carrying strictly more than a count. A structured field needs a coordinated server change first; see the handover note.

## 4. Bounded capture concurrency (spec: step-capture)

- [x] 4.1 Introduce a 2-permit capture semaphore, acquired inside the spawned capture thread after the step number is assigned. Verify with a Rust test that 20 concurrent jobs never exceed 2 in flight and all 20 complete.
- [x] 4.2 Confirm the existing `in_flight` counter still reaches zero before `stop_recording` returns with a queue present; verify with a Rust test that queues jobs behind the semaphore and asserts the stop wait drains them.

## 5. Per-monitor capture (spec: step-capture)

- [x] 5.1 Add pure `monitor_for_click(point: Option<(i32,i32)>, bounds: &[(i32,i32,u32,u32)], primary: usize) -> usize` in `capture/screenshot.rs`. Verify with unit tests for a point on each of three monitors, a layout with a negative-origin monitor left of the primary, a point outside all bounds, and `None`.
- [x] 5.2 Replace `capture_full_screen` with `capture_monitor(index)` returning the single monitor's image and a `VirtualScreen` whose origin is that monitor's origin and whose scale is measured from that monitor alone. Verify existing `canvas_scale` and `to_canvas` tests still pass and add one asserting the origin is the monitor's, not the desktop's.
- [x] 5.3 Wire `capture_and_save` to select the monitor from the click position, with the cursor-position then primary fallback for key-triggered steps; log which display was chosen and when the fallback was used. Verify with a test using synthetic monitor bounds that a right-hand-monitor click yields an image of that monitor's size with the marker at the correct relative offset.
- [x] 5.4 Add the regression test from the design's measurement: two 3840×2160 monitors produce a saved image at least 1600 px wide (today 960). Verify `cargo test` fails on the pre-change code and passes after.

## 6. Click marker (spec: step-capture)

- [x] 6.1 Ring replaces the filled disc, **and the cursor arrow is removed** rather than kept as this task originally said. The arrow was a filled 15x25 px glyph anchored on the click point, so it occluded the control the step exists to identify — the other half of the finding, and the half a ring alone does not fix; it also drew the same glyph whatever the real cursor was. A white hairline is drawn on each edge of the red stroke, inside the existing radius, so the marker reads on dark or red chrome without moving the reported box. Verified: the pixel at the click point is unchanged after rendering.
- [x] 6.2 Add a test asserting a pixel on the ring radius is red, so the marker cannot be silently removed altogether.
- [x] 6.3 Add a test asserting `marker_box_at` returns identical bounds to the pre-change values at scale 1.0 and 2.0, protecting the server-side masking contract.

## 7. Settings load and save (spec: app-settings)

- [x] 7.1 Remove the `keyring_load` call and the `api_key` field from `get_settings` / `AppSettings`; add a `has_api_key() -> bool` command and register it. Verify with a Rust test asserting the credential store is not consulted during a load.
- [x] 7.2 Update the TypeScript `AppSettings` interface, `src/lib/tauri.ts` bindings, and the fixtures in `src/test/settings.test.tsx` and `src/test/errorReportFlow.test.tsx`. Verify `npm run build` and `npm test` pass.
- [x] 7.3 Call `store.save()` in `save_settings` and propagate its error. Verify with a Rust test that writes settings into a tempdir store, then reads `settings.json` **from disk** and finds `skip_pii_check: true`.
- [x] 7.4 Add a `loaded` flag to `SettingsPage`; disable all controls and the save button until the first `getSettings` resolves, and never replace form state from a later async result. Verify with the TS regression test: with `getSettings` pending, toggle and confirm skip-PII, resolve the promise, then assert the toggle is still on and `save_settings` receives `skip_pii_check: true`. This test must fail against the current code.
- [x] 7.5 Add a TS test asserting controls and the save button are `disabled` before the load resolves.
- [x] 7.6 Show a save error in `SettingsPage` and close the window only on success; add German copy. Verify with a TS test that a rejecting `save_settings` leaves the window open, renders the message, and does not call `close()`.

## 8. Log directory (spec: app-settings)

- [x] 8.1 Derive `logs_dir` in `AppSettings::defaults` from `app.path().app_log_dir()`. Verify with a Rust test asserting it equals the resolved log directory and differs from `app_local_data_dir()/logs` wherever the platform separates them.
- [x] 8.2 Overwrite a stored `logs_dir` that disagrees with the real one during `AppSettings::initialize`. Verify with a Rust test that pre-seeds a stale value and asserts it is corrected.
- [x] 8.3 Make the logs field read-only in `SettingsPage` and add a reveal button using `revealItemInDir`; add German copy. Verify with a TS test that the input is `readonly` and the button invokes reveal with the shown path.

## 9. Processing feedback (spec: guide-generation)

- [x] 9.1 Display elapsed processing time in the status area, reusing `useElapsedTime`. Verify with a TS test using fake timers that the display advances.
- [x] 9.2 Show a "still waiting on the server" notice after 20 s without an `sse:status` message, cleared by the next message. Verify with a TS test over both transitions.
- [x] 9.3 Reword a recovering connection as reconnecting rather than interrupted, and raise an error only when the retry budget is exhausted; log each reconnect attempt with its cause in `hooks/useSSE.ts` and `network/sse.rs`. Verify with a TS test covering drop-then-success (no error state) and exhausted retries (error state, reportable).

## 10. Verification and handover

- [x] 10.1 `npm test` (164), `npm run build`, `cargo test` (147) and `cargo build` all pass. `cargo clippy` reports 12 warnings — the same 12 as on `main` before this change, verified by stashing; no new ones introduced. The pre-existing set is not addressed here.
- [~] 10.2 **Single display: done, on 2026-09-06.** A five-step recording against staging, with `step_02.*` removed to stand in for a failed capture, produced a four-step guide. Recorder log: `4 screenshot(s) present, 1 missing from the sequence ([2])`, and no `alignment dropped`. Server log: `steps=4`, `AI enrichment complete, 4 steps`, and `identical adjacent frames: … of 3 pairs (skipped {'unknown_geometry': 0})` — every pair had marker geometry, so `metadata.steps` was accepted and per-step narration survived the gap. The clicked control reads through the ring. **Two displays: still open**, no second monitor available; deferred to the 2026-09-21 session with the tester who reported it.
- [~] 10.3 **macOS: done.** `settings.json` was corrected at startup from `…/Application Support/com.cogniclone.recorder/logs` to `/Users/…/Library/Logs/com.cogniclone.recorder`, which exists and holds `cogniclone.log`. **Windows: still open** — the two paths coincide there, so this is a regression check rather than a fix, but it has not been run.
- [x] 10.4 `CHANGELOG.md` Unreleased section written in the project's user-facing voice, covering the per-monitor capture and marker changes among the rest. The 2026-09-21 agenda note is in the PR description and the handover summary rather than in the repo, since the agenda lives in Notion.

## 11. Found while verifying (2026-09-06)

- [x] 11.1 The guide's image links were built from the step's position, so after a gap every step past it linked the previous step's screenshot and the step at the gap linked a file that was never uploaded. Fixed client-side in `pdf.rs` (`screenshot_for_step`, 4 tests) and server-side in sop-sorcery PR #7 (`sop_template.py`, `sop_export_markdown.py`, 8 tests). Verified by regenerating the same recording against staging: Schritt 2 now links `step_03.png`, and every referenced file exists.
- [ ] 11.2 **Backend, not this change.** `marker_box`, `click_x` and `click_y` reach the server on every step and are used only for the near-duplicate frame comparison (`routes_generate.py:282`); they never enter the generation prompt. So the model has to find the marker by eye and attribute it to a UI element. On a dense sidebar with two near-identical truncated rows it picked the wrong one — the same failure mode as the misread application name in the original report. The data to prevent it is already being sent. Needs its own change: the plumbing is code, but the prompts live in Langfuse and would need authoring and a quality comparison.
