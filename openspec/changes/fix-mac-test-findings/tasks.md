## 1. Diagnostics first

These land before anything else, so the 2026-09-21 session with the tester produces measurements rather than another hypothesis.

- [ ] 1.1 Log monitor count, canvas dimensions and elapsed milliseconds for every capture in `screenshot::capture_and_save`; verify by running a recording and confirming one line per step in the log file.
- [ ] 1.2 Log the resolved screenshot set at the start of `run_generation_inner` (count, first and last step number, any gaps); verify by generating from a folder with a manually deleted `step_02.png` and reading the line back.

## 2. Screenshot enumeration (spec: guide-generation)

- [ ] 2.1 Add `collect_step_screenshots(dir) -> Result<Vec<(u32, PathBuf)>, String>` in `commands/generate.rs`: read the directory, match `step_<n>.png`, parse and sort numerically, log gaps. Verify with a Rust unit test over a tempdir containing `step_01`, `step_03`, `step_04` returning all three in order.
- [ ] 2.2 Add unit tests for the remaining enumeration scenarios: empty directory returns `Err`, `step_10` sorts after `step_09`, non-matching filenames are ignored. Verify `cargo test` passes.
- [ ] 2.3 Replace the counting loop at `generate.rs:90` with the new function; verify the existing generation path still uploads a gapless recording unchanged.
- [ ] 2.4 Log both counts and the words "alignment dropped" when the sidecar count differs from the screenshot count; verify with a unit test asserting the warn-level message is emitted for mismatched inputs.

## 3. Capture failure accounting (spec: step-capture)

- [ ] 3.1 Add `failed_captures: Arc<AtomicU32>` to `RecordingSession`, increment it in the `Err` arm of the capture thread in `commands/recording.rs`, and include it in the value `stop_recording` returns. Verify with a Rust test that drives the counter directly.
- [ ] 3.2 Emit `recording:step_failed` with the running failure count when a capture fails; verify by listening for the event in a dev build with a forced capture error.
- [ ] 3.3 Surface the count in `ReviewScreen` as "N of M steps could not be captured" and add the German copy to `src/i18n/de.ts`. Verify with a TS test asserting the notice renders for `failedSteps=2` and not for `0`.
- [ ] 3.4 Add the failure count to the error-report settings context in `commands/settings.rs::publish_error_report_context`; verify with a Rust test that a report raised after a failure carries it.

## 4. Bounded capture concurrency (spec: step-capture)

- [ ] 4.1 Introduce a 2-permit capture semaphore, acquired inside the spawned capture thread after the step number is assigned. Verify with a Rust test that 20 concurrent jobs never exceed 2 in flight and all 20 complete.
- [ ] 4.2 Confirm the existing `in_flight` counter still reaches zero before `stop_recording` returns with a queue present; verify with a Rust test that queues jobs behind the semaphore and asserts the stop wait drains them.

## 5. Per-monitor capture (spec: step-capture)

- [ ] 5.1 Add pure `monitor_for_click(point: Option<(i32,i32)>, bounds: &[(i32,i32,u32,u32)], primary: usize) -> usize` in `capture/screenshot.rs`. Verify with unit tests for a point on each of three monitors, a layout with a negative-origin monitor left of the primary, a point outside all bounds, and `None`.
- [ ] 5.2 Replace `capture_full_screen` with `capture_monitor(index)` returning the single monitor's image and a `VirtualScreen` whose origin is that monitor's origin and whose scale is measured from that monitor alone. Verify existing `canvas_scale` and `to_canvas` tests still pass and add one asserting the origin is the monitor's, not the desktop's.
- [ ] 5.3 Wire `capture_and_save` to select the monitor from the click position, with the cursor-position then primary fallback for key-triggered steps; log which display was chosen and when the fallback was used. Verify with a test using synthetic monitor bounds that a right-hand-monitor click yields an image of that monitor's size with the marker at the correct relative offset.
- [ ] 5.4 Add the regression test from the design's measurement: two 3840×2160 monitors produce a saved image at least 1600 px wide (today 960). Verify `cargo test` fails on the pre-change code and passes after.

## 6. Click marker (spec: step-capture)

- [ ] 6.1 Replace the filled disc in `render_click_overlay` with a ring drawn at a 3 px stroke scaled by the canvas scale, keeping the white cursor arrow. Verify with a Rust test that the pixel at the click point is unchanged after rendering.
- [ ] 6.2 Add a test asserting a pixel on the ring radius is red, so the marker cannot be silently removed altogether.
- [ ] 6.3 Add a test asserting `marker_box_at` returns identical bounds to the pre-change values at scale 1.0 and 2.0, protecting the server-side masking contract.

## 7. Settings load and save (spec: app-settings)

- [ ] 7.1 Remove the `keyring_load` call and the `api_key` field from `get_settings` / `AppSettings`; add a `has_api_key() -> bool` command and register it. Verify with a Rust test asserting the credential store is not consulted during a load.
- [ ] 7.2 Update the TypeScript `AppSettings` interface, `src/lib/tauri.ts` bindings, and the fixtures in `src/test/settings.test.tsx` and `src/test/errorReportFlow.test.tsx`. Verify `npm run build` and `npm test` pass.
- [ ] 7.3 Call `store.save()` in `save_settings` and propagate its error. Verify with a Rust test that writes settings into a tempdir store, then reads `settings.json` **from disk** and finds `skip_pii_check: true`.
- [ ] 7.4 Add a `loaded` flag to `SettingsPage`; disable all controls and the save button until the first `getSettings` resolves, and never replace form state from a later async result. Verify with the TS regression test: with `getSettings` pending, toggle and confirm skip-PII, resolve the promise, then assert the toggle is still on and `save_settings` receives `skip_pii_check: true`. This test must fail against the current code.
- [ ] 7.5 Add a TS test asserting controls and the save button are `disabled` before the load resolves.
- [ ] 7.6 Show a save error in `SettingsPage` and close the window only on success; add German copy. Verify with a TS test that a rejecting `save_settings` leaves the window open, renders the message, and does not call `close()`.

## 8. Log directory (spec: app-settings)

- [ ] 8.1 Derive `logs_dir` in `AppSettings::defaults` from `app.path().app_log_dir()`. Verify with a Rust test asserting it equals the resolved log directory and differs from `app_local_data_dir()/logs` wherever the platform separates them.
- [ ] 8.2 Overwrite a stored `logs_dir` that disagrees with the real one during `AppSettings::initialize`. Verify with a Rust test that pre-seeds a stale value and asserts it is corrected.
- [ ] 8.3 Make the logs field read-only in `SettingsPage` and add a reveal button using `revealItemInDir`; add German copy. Verify with a TS test that the input is `readonly` and the button invokes reveal with the shown path.

## 9. Processing feedback (spec: guide-generation)

- [ ] 9.1 Display elapsed processing time in the status area, reusing `useElapsedTime`. Verify with a TS test using fake timers that the display advances.
- [ ] 9.2 Show a "still waiting on the server" notice after 20 s without an `sse:status` message, cleared by the next message. Verify with a TS test over both transitions.
- [ ] 9.3 Reword a recovering connection as reconnecting rather than interrupted, and raise an error only when the retry budget is exhausted; log each reconnect attempt with its cause in `hooks/useSSE.ts` and `network/sse.rs`. Verify with a TS test covering drop-then-success (no error state) and exhausted retries (error state, reportable).

## 10. Verification and handover

- [ ] 10.1 Run `npm test`, `npm run build`, `cargo test` and `cargo clippy` clean.
- [ ] 10.2 Build the macOS bundle and record one four-step session on two displays and one on a single display; confirm every click produces a step, the clicked control is readable, and the guide step count matches the click count.
- [ ] 10.3 Confirm `settings.json` reports a log directory that exists and contains the current log file, on both macOS and Windows.
- [ ] 10.4 Update `CHANGELOG.md` with the per-monitor capture behaviour change and the marker change, and note in the 2026-09-21 agenda that the skip-PII hypothesis is disproved so the session tests the capture-failure path instead.
