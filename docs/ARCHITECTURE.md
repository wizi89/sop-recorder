# Architecture

As of 0.19.2 (2026-10-24). For humans and AI agents working on this repo. The generation server and web app are a separate, private codebase; this page covers only what the recorder does and what it sends.

## In one paragraph

A Tauri v2 app: Rust does capture, audio, input hooks, sign-in and networking; a React 19 + TypeScript frontend (German UI) drives it through Tauri commands. A recording is a folder of `step_NN.png` + `step_NN.json` and one WAV file. On **Generate**, the folder is uploaded in one request to the CogniClone API, progress streams back over SSE, and the finished guide is saved next to the recording as `guide.md` and `guide.pdf`. Windows and macOS are supported; Linux does not build (the `keyring` dependency is declared for Windows and macOS only).

## Modules

| Path | Role |
|------|------|
| `src-tauri/src/lib.rs` | Startup: backend flags, logging, panic hook, plugins, the list of registered commands |
| `capture/input_hooks.rs` | Global input: left-click and Return become steps. Windows via `rdev`, macOS via an event tap. Double-clicks on one pixel are deduplicated |
| `capture/screenshot.rs` | Captures the monitor that was clicked, draws a ring marker, scales to at most 1920x1080 PNG |
| `capture/audio.rs` | Microphone to 16 kHz mono WAV via `cpal`, level meter, "no sound" warning |
| `capture/permits.rs` | Bounds concurrent captures |
| `commands/*` | Tauri commands: recording, generate, auth, settings, pipelines, quota, permissions, window, updater, error_reports |
| `network/*` | HTTP: `auth` (sign-in, refresh, keyring), `loopback` (browser sign-in callback), `upload`, `sse`, `jobs` (result polling), `quota`, `pipelines`, `reports` |
| `output/*` | `guide.md`, `pending.json`, per-step metadata |
| `error_reports.rs` | Consent-first error reports with a log scrubber |
| `config.rs`, `runtime_config.rs` | Backend URLs; override via `--local/--staging/--production`, env vars, or `%APPDATA%\CogniClone\config.toml` |
| `src/App.tsx` | Picks the screen: login, permissions, recorder, review |
| `src/hooks/useRecorder.ts` | Frontend state machine: idle, recording, review, processing, done, error, pii_blocked, rate_limited |
| `src/components/RecordingBar.tsx` | The always-on-top recording bar (its own window) |

## Life of a recording

1. **Start** (`commands/recording.rs`): creates a new folder in the output directory (set in Settings), hides the app's own windows from capture, starts audio and input hooks.
2. **Each click or Return**: a capture thread writes `step_NN.png`, then `step_NN.json` (timestamp, click position, trigger, marker box). The UI is told whether it succeeded. **Undo** deletes the last step.
3. **Stop**: hooks and audio stop, in-flight captures get up to 15 s, `pending.json` is written, the review screen opens.
4. **Generate** (`commands/generate.rs`): refreshes the session, uploads everything to `POST /generate` (3 retries, one re-auth on 401), reads SSE progress. If the stream drops, polls `GET /generate/{id}/result`. Saves `guide.md` and downloads the PDF the server rendered; a missing PDF does not fail the guide.
5. **Result**: open the folder. "Generate from folder" re-runs step 4 for any earlier recording.

## Sign-in

Browser sign-in with PKCE: a loopback listener on `127.0.0.1:<random port>`, the browser opens `{webapp}/desktop/authorize`, the code is exchanged at `POST /auth/desktop/exchange`. The refresh token lives in the OS keychain (service `cogniclone`, `cogniclone-dev` for dev builds). The in-app password form (`POST /auth/signin`) stays as a second way in; a failed browser sign-in creates an error report (under the user's report setting). Spec: `openspec/specs/user-authentication/spec.md`.

## What leaves the machine

| When | What | Where |
|------|------|-------|
| Generate | The WAV, every screenshot PNG, title, pipeline choice, per-step timestamp, click position and trigger type | `POST /generate` |
| Sign-in, refresh | Auth code / refresh token | `/auth/*` |
| App start, review screen | Nothing but the session token | `/quota`, `/pipelines` |
| Error report, only with consent | Scrubbed log tail (emails, secrets, paths removed), a subset of settings | `/client-reports` |
| Update check | Nothing personal | GitHub Releases `latest.json` |

Not captured: typed text (only the Return key is detected), UI element names, other mouse buttons, scrolling. Nothing is sent while recording.

Not done on the client today: blurring, password-field detection, active-window-only capture. Screenshots show the whole clicked monitor. The server checks for personal data and can refuse to generate; that check can be switched off in Settings.

## Known gaps

Tracked here until fixed:

- Generate waits for in-flight captures without a deadline (`generate.rs:135`).
- Any failed session refresh, including a server 5xx, signs the user out (`network/auth.rs:418-421`).
- No startup scan for unfinished recordings; `output::pending::find_pending` is unused outside tests.
- The upload is built in memory as one request with a 300 s timeout.
- A missing or failing microphone fails the whole recording.

## Working on it

Commands, tests and release flow: [README](../README.md). Behaviour that surprised us: [FINDINGS.md](./FINDINGS.md). Specs: `openspec/specs/`. CI runs vitest, `cargo test --lib`, `cargo fmt --check`, `clippy -D warnings` and `tsc` on Windows and macOS; keep all of them green.
