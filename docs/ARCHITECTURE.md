# Architecture

As of 0.19.2 (2026-10-24). For humans and AI agents working on this repo. The generation server and web app are a separate, private codebase; this page covers only what the recorder does and what it sends.

## In one paragraph

A Tauri v2 app: Rust does capture, audio, input hooks, sign-in and networking; a React 19 + TypeScript frontend (German UI) drives it through Tauri commands. A recording is a folder of `step_NN.png` + `step_NN.json` and one WAV file. On **Generate**, the folder is uploaded in one request to the CogniClone API, progress streams back over SSE, and the finished guide is saved next to the recording as `guide.md` and `guide.pdf`. Windows and macOS are supported; Linux does not build (the `keyring` dependency is declared for Windows and macOS only).

## Modules

| Path | Role |
|------|------|
| `src-tauri/src/lib.rs` | Startup: backend flags, logging, panic hook, plugins, the list of registered commands |
| `capture/input_hooks.rs` | Global input: left-click and Return become steps. Windows via `rdev`, macOS via an event tap. Double-clicks on one pixel are deduplicated |
| `capture/screenshot.rs` | Captures a step in two phases: reads the clicked monitor and crops it to the capture area, then masks, draws a ring marker, scales to at most 1920x1080 and writes the PNG |
| `capture/window_area.rs` | Pure geometry: the active window area (foreground window, its owner, popups under the click), which windows are visible in it |
| `capture/geometry.rs` | Windows only: the window snapshot (`FindWindowExW` walk, DWM bounds) taken before and after each pixel read |
| `capture/password_check.rs` | The password check: cached and fresh field queries per window within one budget, and every case that flags a step |
| `capture/uia.rs` | Windows only: UI Automation query for `IsPassword` fields, and the focus/show/restore events that warm the check up |
| `capture/audio.rs` | Microphone to 16 kHz mono WAV via `cpal`, level meter, "no sound" warning |
| `capture/permits.rs` | Buffer slots (2 to 6 by monitor size, about 100 MB in total) and capture permits that serialise pixel reads |
| `commands/*` | Tauri commands: recording, generate, auth, settings, pipelines, quota, permissions, window, updater, error_reports |
| `network/*` | HTTP: `auth` (sign-in, refresh, keyring), `loopback` (browser sign-in callback), `upload`, `sse`, `jobs` (result polling), `quota`, `pipelines`, `reports` |
| `output/*` | `guide.md`, `pending.json`, per-step metadata, `redactions.json` (the review screen's boxes, `output/redactions.rs`) |
| `commands/review.rs` | What the review screen changes before upload: saves the boxes per step, deletes a step |
| `error_reports.rs` | Consent-first error reports with a log scrubber |
| `config.rs`, `runtime_config.rs` | Backend URLs; override via `--local/--staging/--production`, env vars, or `%APPDATA%\CogniClone\config.toml` |
| `src/App.tsx` | Picks the screen: login, permissions, recorder, review |
| `src/hooks/useRecorder.ts` | Frontend state machine: idle, recording, review, processing, done, error, pii_blocked, rate_limited |
| `src/components/RecordingBar.tsx` | The always-on-top recording bar (its own window) |
| `src/lib/redactionGeometry.ts` | Maps review-screen boxes between display pixels and saved-image pixels |

## Life of a recording

1. **Start** (`commands/recording.rs`): creates a new folder in the output directory (set in Settings), hides the app's own windows from capture, starts audio and input hooks.
2. **Each click or Return**: a capture thread takes a window snapshot, reads the monitor's pixels, takes a second snapshot and crops to the capture area (the active window by default, the whole monitor in "Ganzer Bildschirm" mode). It then runs the password check, fills password fields black in memory, and writes `step_NN.png`, then `step_NN.json` (timestamp, click position, trigger, marker box, `capture_scope`, `password_check`, and `window_list_partial` when the window list left out shell windows). No unmasked file is written. The UI is told whether it succeeded. **Undo** deletes the last step. Capture area and masking are Windows only; macOS captures the monitor without masking.
3. **Stop**: hooks and audio stop, in-flight captures get up to 15 s, `pending.json` is written, the review screen opens. Steps whose password check was not confirmed (`flagged`, `unknown`) or that fell back to the monitor or came from a partial window list (with masking on or off) carry a "Prüfen" mark, and Generate needs one confirmation tick. On the review screen the user can draw boxes to black out (saved to `redactions.json`, burned into the uploaded copy only; the PNGs on disk stay the originals) and delete steps (the other steps keep their numbers).
4. **Generate** (`commands/generate.rs`): refreshes the session, uploads everything to `POST /generate` (3 retries, one re-auth on 401), reads SSE progress. If the stream drops, polls `GET /generate/{id}/result`. Saves `guide.md` and downloads the PDF the server rendered; a missing PDF does not fail the guide.
5. **Result**: open the folder. "Generate from folder" re-runs step 4 for any earlier recording.

## Sign-in

Browser sign-in with PKCE: a loopback listener on `127.0.0.1:<random port>`, the browser opens `{webapp}/desktop/authorize`, the code is exchanged at `POST /auth/desktop/exchange`. The refresh token lives in the OS keychain (service `cogniclone`, `cogniclone-dev` for dev builds). The in-app password form (`POST /auth/signin`) stays as a second way in; a failed browser sign-in creates an error report (under the user's report setting). Decision: [ADR 0006](./adr/0006-browser-sign-in-over-loopback.md).

## What leaves the machine

| When | What | Where |
|------|------|-------|
| Generate | The WAV, every screenshot PNG not deleted on the review screen (cropped and masked as captured, with the review screen's boxes burned in), title, pipeline choice and generation model, per-step timestamp, click position, trigger type, marker box, capture scope (`capture_scope`) and password check outcome (`password_check`), whether the step's window list was partial (`window_list_partial`, only when it was), per-recording counts of capture scopes (`capture_scopes`) and password check outcomes (`password_checks`), and how many steps came from a partial window list (`partial_window_lists`) | `POST /generate` |
| Sign-in, refresh | Auth code / refresh token | `/auth/*` |
| App start, review screen | Nothing but the session token | `/quota`, `/pipelines` |
| Error report, only with consent | Scrubbed log tail (emails, secrets, paths removed), a subset of settings | `/client-reports` |
| Update check | Nothing personal | GitHub Releases `latest.json` |

Not captured: typed text (only the Return key is detected), UI element names, other mouse buttons, scrolling. Nothing is sent while recording.

On Windows, screenshots show the active window by default, and password fields that UI Automation reports (`IsPassword`) are blacked out before the PNG is written. Windows of other apps drawn over the area (notifications, the notification center, quick settings, always-on-top windows) are blacked out where they overlap it; click-through overlays and the window the click lands on stay visible. A field a "show password" toggle has switched to plain text is not a password field to UI Automation and is not masked. Not done on the client today: blurring, and any masking on macOS (there, screenshots show the whole clicked monitor). The server checks for personal data and can refuse to generate; that check can be switched off in Settings.

The same for users, in German: [PRIVACY.md](./PRIVACY.md).

## Known gaps

Tracked here until fixed:

- Generate waits for in-flight captures without a deadline (`generate.rs:135`).
- Any failed session refresh, including a server 5xx, signs the user out (`network/auth.rs:418-421`).
- The upload is built in memory as one request with a 300 s timeout.
- A missing or failing microphone fails the whole recording.

## Working on it

Commands, tests and release flow: [README](../README.md). Behaviour that surprised us: [FINDINGS.md](./FINDINGS.md). Domain terms: [GLOSSARY.md](../GLOSSARY.md). Decisions: [docs/adr/](./adr/). CI runs vitest, `cargo test --lib`, `cargo fmt --check`, `clippy -D warnings`, `tsc` and Biome's format check on Windows and macOS; keep all of them green.
