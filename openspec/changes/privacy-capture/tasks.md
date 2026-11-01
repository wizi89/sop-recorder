## 1. Spike: UIA password query (gate for sections 3-4)

- [x] 1.1 Throwaway binary (not committed) that finds `IsPassword` elements in given windows and prints count, rectangles and time; at least 20 runs per case: Chrome and Edge login page, Firefox, one Electron app (Teams or Slack), a Win32 dialog with a password box, an elevated window (Firefox and an Electron login not available; moved to 8.1)
- [x] 1.2 First query on a window just created or just raised by a click vs repeated queries (Chrome, Edge, Electron, Win32): decide the D5 exemptions per window class
- [x] 1.3 Submit by Enter in the password field and by clicking the submit button: what a query 0-500 ms after the event returns; plus a login page loaded inside an existing tab, filled by keyboard and submitted by Enter: do UIA focus-changed events report the password field and its rectangle
- [x] 1.4 Monitor mode on a realistic desktop (10+ windows, several Chromium, Outlook, an IDE, one elevated): total query time and resulting flag rate (measured on a 14-window desktop without Outlook or an elevated window; the real desktop is in 8.1)
- [x] 1.5 Record results (hit rate, p50/p95, flag rate per case) in design.md, set the budgets, exemptions and processing permit count; pass bar: every field found in non-elevated cases (counting cached rectangles per D5), p95 within budget, flag rate on ordinary work low enough that the confirmation stays meaningful. If the bar is not met, stop here and bring the results to the user before section 3
- [x] 1.6 Note the `windows` crate features the spike needed, for task 3.1

## 2. Settings

- [x] 2.1 `capture_mode` and `mask_passwords` with defaults `active_window` / `true` in `get_settings`, `save_settings` and the recording-start read for keys missing from the store; tests through those paths with a store lacking the keys (design D8)
- [x] 2.2 `SettingsPage.tsx` section "Datenschutz bei der Aufnahme": two radio options with the monitor warning, the masking checkbox with "Passwortfelder werden mit aufgenommen." when off; macOS shows the monitor option only, no masking checkbox, plus the note; strings in `i18n/de.ts` with real umlauts
- [x] 2.3 Read both settings once at recording start into recording state; test that a change during a recording does not affect it

## 3. Capture phases and the window area (Windows)

- [x] 3.1 Enable the `windows` crate features from 1.6 (at least `Win32_Graphics_Dwm`, `Win32_UI_Accessibility`, `Win32_System_Com`)
- [x] 3.2 Split capture into phase A (pixels, geometry snapshot, crop to area, under a new 2-permit capture pool) and phase B (check, mask, save), with the existing `permits.rs` pool raised to 6 as buffer slots held across both phases and the in-flight counter covering both, per design D3. Tests with a mock check: five quick steps against a slow check are not delayed; twenty against a hanging check never hold more than six buffers and all twenty are saved; Stop waits for a phase B blocked on a slow check
- [x] 3.3 Geometry snapshot before and after the pixel read (design D3): foreground window, window under the click, root owners, DWM extended frame bounds, z-ordered visible top-level windows with cloaked/layered/transparent marked; visible in either counts as visible; flag when the foreground differs; test with two synthetic snapshots
- [x] 3.4 Pure function for the area (design D2): bounding box of window and owner clipped to the monitor, gap fill, desktop as no window, other shell UI (taskbar, Start, tray flyouts) as the step's window, `monitor_fallback`; tests for normal, dialog away from owner, partly off screen, desktop click, taskbar click, click on a background window, click on a context menu item reaching past the window edge
- [x] 3.5 Apply area before the marker; `marker_box` translated by crop offset and downscale, `click_x`/`click_y` left in desktop coordinates (design D7a); tests that the clicked pixel survives and the marker box matches after crop and downsize
- [x] 3.6 Log one line per step: mode, area or fallback reason

## 4. Password check (Windows)

- [x] 4.1 Pure function for each window's visible part and the visible part of each found field (opaque windows only, design D4); tests for a field behind an opaque window, behind a layered window, behind a cloaked window, behind a window excluded from capture (the recording bar)
- [x] 4.2 UIA query workers (MTA) with the per-step budget from 1.5 and the abandoned-query cap (design D6); tests with a mock that hangs: two abandoned, the third step skips and is flagged; elevated or unopenable window processes are not queried and flag the step (amendment A1)
- [x] 4.3 Warm-up and cache (design D5): all visible windows at recording start, `SetWinEventHook` for foreground and show events, UIA focus-changed events for `IsPassword` elements; warm-ups through the same capped workers; union of cached and fresh results; a window counts as checked after two completed queries at least 100 ms apart, warm-ups query twice, an empty or zero-sized field rectangle flags (amendment A2); flag on a window not yet checked and on disagreement; tests for each flag rule
- [x] 4.4 Translate the visible field rectangles by the crop offset and fill them on the cropped in-memory image (design D3); no unmasked image is written anywhere, including temp files
- [x] 4.5 `StepMeta.capture_scope` and `password_check`; missing reads as `unknown`; serialisation round-trip tests

## 5. Review screen

- [x] 5.1 Tauri command returning `capture_scope` and `password_check` per step, with a missing sidecar as `unknown`; listed in `lib/tauri.ts`
- [x] 5.2 `ReviewScreen.tsx` marks steps per design D7; the confirmation "Ich habe die markierten Schritte geprüft" shown only with marked steps; Generate disabled until ticked; component tests for marked, unmarked and legacy recordings

## 6. Upload metadata

- [x] 6.1 `upload.rs`: `capture_scopes` and `password_checks` counted per uploaded screenshot, a missing sidecar or field as `unknown` (design D9); tests for a normal, a regenerated, a legacy folder and one screenshot without sidecar
- [x] 6.2 `sop-sorcery` server: new log line after metadata parsing in `/generate` with both objects, plus trace metadata (separate PR into master there) — PR wizi89/sop-sorcery#24

## 7. macOS

- [ ] 7.1 Capture path ignores both settings on macOS and records `capture_scope: monitor`, `password_check: off`; existing macOS tests still pass

## 8. Verification

- [ ] 8.1 Manual Windows run: maximised and windowed browser, a Save As dialog moved away from its owner, a context menu, click on a background window, desktop click, taskbar click, two monitors, a login page submitted by Enter and by click with masking on, a login page loaded inside an existing tab and filled by keyboard, a click that activates a window partly covered by another, masking off, an elevated window in the area, an always-on-top window over the captured window; plus the spike gaps from 1.1/1.4: a Firefox login page, an Electron app's login screen, and monitor mode on the real working desktop with Outlook, the IDE and one elevated window
  - 2026-11-01 run (4 recordings, 57 steps): every area and every mask correct, Electron (Bitwarden) login masked. Flag rate 8/15, 11/15, 0/7 (off), 8/20 led to five fixes: menus not checked, windows checked side by side, warm-up on restore from minimised, foreground change flags only window-mode steps that are not the taskbar alone, a window closed after the read keeps its cached result. Going through the logs step by step (not re-run), the flags that would remain are the intended ones (desktop, elevated window, login steps whose fields changed) plus 3/20 short-lived windows never checked. Query time p50 66 ms, p90 323 ms over 192 queries. Open: a Firefox login page, Outlook not identified in the log, and a short rerun to confirm the fixes live.
- [ ] 8.2 Offline comparison: record three short workflows in monitor mode with the active-window area logged per step, derive the cropped variant of each from the same files, generate each variant three times on staging (cost estimate first); ship the active window default unless the cropped guides name the wrong element or lose a step more often than the monitor guides in a blind comparison
- [x] 8.3 CHANGELOG entry: new defaults, review marks, and how to switch back
