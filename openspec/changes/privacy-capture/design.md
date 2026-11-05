# Design

## Context

`capture::screenshot::capture_and_save` captures the whole monitor containing the click (xcap), draws the click marker, downsizes to at most 1920x1080 and writes `step_NN.png`; the sidecar `step_NN.json` follows, and a failed sidecar write is only logged (`recording.rs:287-292`). Capture runs on a spawned thread per input event; the thread holds one of `DEFAULT_CAPTURE_PERMITS = 2` (`capture/permits.rs`) as an RAII guard for the whole of `capture_and_save`. Steps fire on mouse-down and Enter from the low-level hook (`input_hooks.rs`), before the click's own effect (activation, navigation) has happened. The process is per-monitor DPI aware (`main.rs`), so on Windows the click position, window rectangles and xcap pixels share one physical-pixel space. `pending.json` is written at stop and deleted by `clear_pending` after a successful generation; "generate from folder" (`App.tsx`) re-uploads folders without it. Settings are read key by key with `unwrap_or` in `get_settings`, `save_settings` and `start_recording` (`settings.rs`, `recording.rs`); `initialize` never writes new keys for existing installs. The server's `step_similarity.py` compares `click_x`/`click_y` of consecutive steps in desktop space and skips pairs of different image size. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**
- One capture pipeline for both modes.
- The only screenshot file ever written is the masked one.
- Every case where the check may have missed a field ends as a flag the user sees, never as `ok`.
- A slow or hung password check never delays the pixels of any step, its own or later ones.

**Non-Goals:**
- True per-window capture (PrintWindow / Windows.Graphics.Capture).
- Masking anything other than UIA password fields (OCR, address bars: later, if at all).
- Live re-reading of settings during a recording.

## Decisions

**D1. Crop the monitor capture rather than capture the window.** The monitor is still captured with xcap, then cut down to the window area (D2). Window capture APIs render one window alone, so a dropdown or context menu (a separate popup window) would vanish from the step that shows it being used. Cost: any window drawn above the captured windows inside their area (a toast, an always-on-top window, another app overlapping the owner) stays visible. The spec states this exception; such windows are inside the password check's scope (D4).

**D2. The captured area is the foreground window plus its root owner; the rest of their bounding box is filled.** From the geometry snapshot (D3): foreground window, `GetAncestor(GA_ROOTOWNER)`, bounds from `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` (excludes the invisible resize border). If the click lies outside both, look at the root window under the click (`WindowFromPoint`). If it is a popup or menu of the foreground window's thread or process (a context menu `#32768` or a dropdown reaching past the window edge), add it to the area together with the foreground window and its owner. Otherwise use it and its root owner instead; this covers click-to-activate, where the hook fires before the clicked window becomes foreground. The image is cropped to the bounding box of the two rectangles, clipped to the click's monitor, and pixels inside the box but outside both rectangles are filled with the mask colour, so the gap between a dialog and a distant owner shows nothing. The desktop (`Progman`, `WorkerW`) counts as "no window". A click on other shell UI (taskbar `Shell_TrayWnd`, Start menu, tray flyouts) captures that shell window as the step's window: the user is operating it, so it is what the step documents. With no usable window the whole monitor is kept: the step's scope is `monitor_fallback`, it is logged, and the review screen marks it (D7); the password check then covers the whole monitor as in monitor mode. Alternative considered: owner only when the foreground window is a dialog; the union covers both cases with one rule.

**D3. Two phases per step; the slow one does not hold the capture permit.**
- Phase A (capture permit): take a geometry snapshot, read the monitor pixels, then immediately a second geometry snapshot. A window counts as visible where it is visible in either snapshot, and the step is flagged if the foreground window differs between them (activation completed during the pixel read). Each snapshot holds: foreground window, window under the click, their root owners and rectangles, and the z-ordered list of visible top-level windows on that monitor with their rectangles (`EnumWindows`). Compute the area (D2, geometry only, no UIA) and crop the image to it at once, so a held image is the area, not the monitor. Release the capture permit.
- Phase B: run the password check (D4, D5), mask, fill the gap, draw the marker, downsize, write the PNG, write the sidecar.
- Memory bound: two pools. The existing pool in `capture/permits.rs` (`DEFAULT_CAPTURE_PERMITS`, raised from 2 to 6) becomes the buffer slots: a step holds one from before its pixels are read until its PNG is written. A new pool of 2 capture permits, taken after the slot, only serialises the pixel reads. Field rectangles found in phase B are translated by the crop offset and filled on the cropped image. When all 6 slots are held, the next step waits for one before reading its pixels, as every step does today when the pool is exhausted; the wait is logged with its length. So at most 6 images exist at once, and the check delays no screenshot until 6 steps are in flight.
- The in-flight counter that `stop_recording` and the generate path wait on covers both phases and drops only after the sidecar is written.
The check runs after the pixels by design: what it must protect is what the pixels show.

**D4. The check covers every window visible in the captured area, in both modes.** The windows checked are all windows from the phase A snapshot with a visible part in the final area (active window mode) or on the monitor (monitor mode); a window's visible part is its rectangle minus the rectangles of opaque windows above it in the z-order. A found field is masked only where it is visible, so a password field of a window hidden behind another does not black out the window in front. Only windows that certainly hide what is below count as opaque: cloaked windows (`DWMWA_CLOAKED`, e.g. other virtual desktops) layered or transparent windows (`WS_EX_LAYERED`, `WS_EX_TRANSPARENT`), and windows excluded from capture (display affinity other than `WDA_NONE`, which includes the recorder's own recording bar when `hide_from_screenshots` is on) never hide anything in this calculation, so an error here over-masks rather than leaks. Cloaked windows are not drawn, so they are not checked either. For each: `ElementFromHandle`, then `FindAll(TreeScope_Descendants, IsPasswordProperty == true)`, reading each `BoundingRectangle` (physical screen pixels). Queries run on worker threads with MTA COM, with a total budget per step (initially 500 ms in active window mode; monitor mode gets its own budget from the spike), `IUIAutomation2` connection and transaction timeouts set below it. A window whose query errors or does not finish in the budget makes the step flagged; fields found so far are still masked. An elevated window does not refuse the query: it returns no fields and no error (spike). So a window whose process the recorder cannot open with `PROCESS_QUERY_LIMITED_INFORMATION` and read the token of, or whose token is elevated while the recorder's is not, is not queried at all and flags the step (amendment A1).

**D5. Masking uses the union of cached and fresh results, and flags disagreement.** A per-window cache holds the latest completed queries of each window, each with its time and the password rectangles it found. A step uses what was known when its pixels were read: the last query before the read, plus every query after it. A query that finishes after the read (a submitted login whose field is gone) therefore cannot erase a rectangle the pixels still show (amendment after review: the cache first kept only the newest result, so a warm-up finishing between the read and the check could unmask the field silently). The cache is filled by warm-up queries: at recording start for every visible top-level window, and on each `EVENT_SYSTEM_FOREGROUND` and top-level `EVENT_OBJECT_SHOW` (`SetWinEventHook`) during recording. Because a page can change inside one window without either event (in-tab navigation to a login page), the recorder also subscribes to UIA focus-changed events and adds the rectangle of every focused `IsPassword` element to its window's cache: typing a password requires focusing the field. A focused rectangle stays until two queries after the focus, both before the read and `WARM_UP_GAP` apart, have not found it; a later empty query alone does not remove it (A2). Warm-up queries run on the same workers, under the same budget and abandoned-query cap as step queries (D6). For each step, masked = cached rectangles of checked windows union the fresh results. Rules:
- A window is checked only after two completed queries at least 100 ms apart before the step's pixel time; otherwise the step is flagged. A first query cannot tell "no fields" from "tree not built yet": in the spike, Chromium's first query on a new or raised window returned 0 fields with no error and the second found the field. Warm-ups therefore query twice (amendment A2). No window class is exempt (spike results below).
- A field whose rectangle is empty or zero-sized is still being built: the step is flagged (A2).
- If cached and fresh results differ for a window (a field vanished after Enter or a submit click, the page navigated), mask the union and flag the step. The cached rectangle still covers the field the pixels show.
Over-masking a stale rectangle is the accepted cost; the flag sends the user to look.

**D6. Abandoned queries are capped.** A query that outlives the budget is abandoned, not killed (UIA calls cannot be cancelled). An in-flight counter tracks abandoned queries; while it is at its cap (2), steps skip the query entirely and are flagged. A step does not query a window whose earlier query is still running past its deadline; it flags that window instead, so the threads stuck in a hung window stop growing once its first query is overdue. A warm-up query counts as abandoned only after its own, longer deadline (5 s), because at recording start every visible window is warmed up at once and ordinary slow warm-ups would otherwise fill the cap. Steps check their windows side by side, so a hung app with several windows in one step's area can hold more than two threads; the cap then stops all new queries. The recording carries on with every affected step visible to the user. (Amended after review: the cap counted warm-ups against the step budget, and a step started a new query on a window that was already stuck.)

**D7. Every step records what actually happened, in its sidecar.** `StepMeta` gains `capture_scope` (`window` | `monitor` | `monitor_fallback`) and `password_check` (`ok` | `flagged` | `off`). Missing fields read as `unknown`. A step is marked on the review screen when `password_check` is `flagged` or `unknown`, or `capture_scope` is `monitor_fallback` or `unknown`; a step whose sidecar is missing counts as `unknown` on both. A new Tauri command returns these per step to the review screen.

**D7a. Click coordinates keep their meaning.** `click_x`/`click_y` stay in desktop coordinates, as today, because the server's step-similarity pass compares them between consecutive steps and per-step crops differ. `marker_box` stays in saved-image pixels and is translated by the crop offset and the downscale.

**D8. Settings.** `capture_mode` (`active_window` | `monitor`) and `mask_passwords` are new settings, defaulting to `active_window` and `true` in every read path (`get_settings`, `save_settings`, the recording-start read) for keys missing from the store. They are read once at recording start and held in recording state; changing them affects the next recording. The settings are not persisted per recording: the sidecars (D7) are the record.

**D9. Upload metadata derived from the sidecars.** `capture_scopes` and `password_checks` are objects counting the uploaded screenshots per value, a screenshot without a sidecar or field counting as `unknown`, computed at upload time, so a regenerated or legacy folder reports what its files say and never a default. The server adds a log line after metadata parsing and puts both on the trace metadata; no behaviour change. Window crops vary in size, so the server's advisory step-similarity pass will skip more pairs as `dimension_mismatch`; the log line lets its numbers be split by scope.

**D10. macOS unchanged.** On macOS the capture path ignores both settings, `capture_scope` is `monitor`, `password_check` is `off`, and the UI shows the monitor option only, no masking checkbox, with a note that window capture and password masking follow later.

## Risks / Trade-offs

- [Flag rate too high, so the acknowledgement becomes a reflex] → the spike measures flag rate per case; D5 exemptions and budgets are set from it; if the rate on ordinary work stays high, the design comes back to the user before task 4.
- [Chrome accessibility mode slows the browser while recording] → measured in the spike.
- [Windows drawn over the captured area stay visible] → stated in the spec and the privacy text; they are checked for password fields (D4).
- [Elevated windows in the area flag every step] → honest and visible; the privacy text names it.
- [Stale cached rectangle masks content that is no longer a password field] → the step is flagged, the user sees it.
- [Six held buffers instead of two raise peak memory: in monitor mode up to about 200 MB on one 4K display] → still a fixed bound, unlike the unbounded composites the pool was introduced for; active window mode holds only cropped areas; the slot count is a constant that the spike's burst measurement may lower.
- [Small crops lose context the model needs] → root-owner union (D2); measured offline in task 8.2.

## Migration Plan

Ships in a normal recorder release. Existing Windows installs start recording in active window mode with masking on after the update; the CHANGELOG says so and how to switch back. Recordings made before the update report `unknown` and are marked on the review screen. The server-side log change is independent. Rollback: monitor mode and masking off in the settings; a recorder rollback restores today's behaviour.

## Open Questions

- None left from the spike. Firefox, an Electron login screen and a real desktop with Outlook and an elevated window were not measured; they are checked in task 8.1 against the rules above.

## Spike notes

**`windows` crate features (task 1.6).** The spike built against `windows` 0.58, the recorder's version, with `Win32_Graphics_Dwm` (cloaked state, extended frame bounds), `Win32_UI_Accessibility` (UIA, `SetWinEventHook`), `Win32_System_Com` (MTA, `CoCreateInstance`), `Win32_System_Variant` (the `IsPassword` condition value) and `implement` plus a `windows-core` 0.58 dependency (the focus-changed event handler), on top of the recorder's existing four. `Win32_Security` and `Win32_System_Threading` were only for the spike's elevation readout, and `Win32_UI_Input_KeyboardAndMouse` only for its own Enter hook; the recorder needs neither for D2-D6.

**Results (tasks 1.1-1.5, 2026-11-01, Windows 11, one 4K-class desktop).** 20 queries per case unless stated; times are one `ElementFromHandle` + `FindAll` on an MTA thread.

| Case | Field found | First query | p50 / p95 |
|---|---|---|---|
| Chrome 156, test login page | 19/20 | 0 fields, 150 ms | 47 / 93 ms |
| Edge, test login page | 19/20 | 0 fields, 127 ms | 57 / 127 ms |
| Brave 1.98 (page already open) | 20/20 | 1 field, 243 ms | 52 / 78 ms |
| Win32 CredUI dialog (`#32770`) | 20/20 | 1 field, 179 ms | 62 / 78 ms |
| Same dialog, elevated | 0/20, **no error** | 0 fields, 48 ms | 2 / 23 ms |
| Desktop, 14 windows, sequential | - | - | 782 / 1049 ms per step |

Not measured: Firefox (not installed), an Electron login screen (Teams signed in). Other Electron/Chromium windows (VS Code, Orca, Aqua Voice) answered in 20-60 ms with no fields; Explorer (`CabinetWClass`) is the slowest common window at about 180 ms.

Watch mode (1.2, 1.3):
- A Chromium window's first query after it is created or raised often returns 0 fields and is complete: no error, no timeout. The second query 100-500 ms later finds the field. A newly inserted field can first report an empty rectangle (`0,0 0x0`).
- Enter in the password field: the field is gone at +0 ms after the key event, in every run. Click on the submit button: present at +0 ms, gone at +100 ms. A query after the event therefore never covers a submit; the cached rectangle does.
- UIA focus-changed events reported the password field with its correct rectangle on every focus, by click and by Tab, including the login form inserted into an existing tab without any window event.
- Errors seen: `0x80131505` and `0x80040201` on a Chrome window closing during the query; both are ordinary failures and flag.

**Budgets and counts.** Active window mode: 500 ms per step (one to three windows, first queries up to about 400 ms). Monitor mode: 2000 ms per step (at 1000 ms, 2 of 20 steps on a 14-window desktop would be flagged, at 2000 ms none); parallel workers would lower it but are not needed to pass. Buffer slots stay at 6. Abandoned-query cap stays at 2.

**D5 exemptions.** None. Chromium windows are the opposite of exempt (first query empty); Win32 dialogs answered completely on the first query, but one class is too little evidence to exempt anything.

**Pass bar.** Met, provided the two amendments below are adopted: every field in the non-elevated cases is covered by a cached or fresh rectangle, p95 is within budget in both modes, and with a two-query warm-up the only flags on ordinary work are the first step on a newly opened window and failures.

**Amendments (adopted 2026-11-01, folded into D4 and D5 above).**
- *A1, to D4:* an elevated window returns no fields and no error, so a failed or refused query is not how it shows. A window whose process the recorder cannot open for `PROCESS_QUERY_LIMITED_INFORMATION` with a readable token, or whose token is elevated while the recorder's is not, is treated as not checkable: the step is flagged and no query is run. Needs `Win32_Security` and `Win32_System_Threading` after all, contrary to the feature note above.
- *A2, to D5:* a window counts as checked only after two completed queries at least 100 ms apart. Warm-ups therefore query twice. A rectangle that is empty or zero-sized counts as a field still being built: the step is flagged.
