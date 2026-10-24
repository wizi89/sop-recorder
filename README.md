# SOP Recorder

Desktop app that captures screen clicks and audio narration, then generates polished SOP (Standard Operating Procedure) documents using AI.

Built with [Tauri v2](https://tauri.app/) (Rust + React + TypeScript).

## Download

Download the latest installer from [GitHub Releases](https://github.com/wizi89/sop-recorder/releases/latest). On Windows, run the `.exe` -- no dependencies required, no admin rights needed. On macOS, download the `.dmg`, open it, and drag the app into Applications.

### First run on macOS

The macOS app is signed with cogniclone's Apple Developer ID and notarized by
Apple, so it opens by double-clicking like any other program.

**Three permissions are needed, and the app asks for them on first run.** The
microphone is granted in a dialog; Screen Recording and Accessibility are
switches in System Settings, and the app links straight to each pane. Screen
Recording is the one permission macOS reports only to a freshly started
process, so the app keeps showing it as missing until you use **App neu
starten** -- the button is on the permission screen for exactly this.

**Permissions survive updates.** macOS ties them to the certificate that signed
the app, and every release is signed with the same one. If CogniClone is ever
missing from the Screen Recording list, add it by hand with **+** at the bottom
of that list.

### Signing in

**Mit Browser anmelden** opens your web browser, where your password manager can fill in
the form -- or, if you are already signed in on the website, you only confirm.
The app never sees your password. For one more version the old form is still
available behind **Ohne Browser anmelden**.

## How it works

1. Click **Aufnahme starten** (Start Recording)
2. Perform your workflow -- every mouse click is captured as a screenshot
3. Narrate what you're doing -- audio is recorded simultaneously
4. Click **Aufnahme stoppen** (Stop Recording)
5. The app uploads screenshots + audio to the server, which uses AI (Whisper + GPT-4o) to generate a step-by-step guide
6. A markdown file and PDF are saved locally

## Features

- Screen capture on mouse click with click position overlay
- Audio recording (16kHz mono WAV, resampled from the device's native rate through an anti-aliasing filter)
- Single running instance: launching the app again brings the running one to the front
- Server-side AI pipeline with SSE progress streaming
- Local PDF + markdown generation
- Sign-in through the user's browser (loopback redirect with PKCE), session in the OS keyring
- Crash recovery (pending.json retry)
- Auto-updater via GitHub Releases, with an opt-in beta channel
- System tray integration
- German UI (i18n)
- Per-user NSIS installer on Windows (no admin rights)
- macOS `.dmg` and `.app` bundles

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) 24 (pinned in `.nvmrc`)
- [Rust](https://rustup.rs/) stable
- Windows 10/11 for Windows installers
- macOS 14+ with Xcode Command Line Tools for macOS bundles

### Setup

```bash
npm install
```

### Run in dev mode

```bash
npm run app             # whatever backend Settings says
npm run app:staging     # against staging, without touching Settings
npm run app:local       # against a server on localhost:8000
npm run app:production
```

The dev build shares its single-instance lock with the installed app. While an installed CogniClone is running, the dev build brings that one to the front and quits -- quit the installed app first.

#### Choosing a backend without opening Settings

`--local`, `--staging` and `--production` point the app at that backend for one
run. They override `upload_target` and leave it alone, so a run started with
`--staging` does not change what the app does the next time it is opened
normally. Giving two different backends at once is refused rather than resolved,
because recording against production while believing you are on staging is the
confusion these flags exist to remove.

The app says which backend it is on, on the terminal and in the log:

```
cogniclone: backend overridden on the command line: --staging (https://api.staging.cogniclone.ai)
```

A packaged build takes the same flags -- useful for testing the real `.app`
against staging, which is not something Settings can express:

```bash
/Applications/cogniclone.app/Contents/MacOS/cogniclone --staging
```

This is a convenience spelling of the `COGNICLONE_API_URL` and
`COGNICLONE_WEBAPP_URL` variables that already existed, so it grants nothing new
-- which is why it is not restricted to debug builds. Precedence, highest first:
the flag, those variables, `config.toml`, then the compile-time default. See
`src-tauri/src/runtime_config.rs`.

#### macOS: stop the Keychain asking for your password on every rebuild

The recorder keeps your login in the macOS Keychain. The Keychain grants access
to a *code identity*, and a dev build is ad-hoc signed -- so that identity is
just the binary's hash, which changes on every `cargo build`. Each rebuild looks
like a new program that was never granted access, and macOS asks for your
password again. "Always Allow" only whitelists the one build it was clicked for.

Signing every dev build with one stable certificate fixes it:

```bash
./scripts/macos-dev-cert.sh   # once per machine
```

This creates a self-signed "CogniClone Dev" code-signing identity in your login
keychain. `src-tauri/.cargo/config.toml` then routes `cargo run` and `cargo test`
through `scripts/macos-dev-sign-run.sh`, which signs the binary with it before
running. The next keychain prompt is the last one -- click "Always Allow".

The setup is local-only and fail-safe: without the identity the runner execs the
binary unsigned, exactly as cargo would, so CI and fresh clones are unaffected.
It does nothing for shipped builds, which need a real Developer ID certificate
and notarization (see below).

### Run tests

```bash
npm test                          # Frontend tests (vitest)
cd src-tauri && cargo test --lib  # Rust tests
npm run e2e:build && npm run e2e  # The real app, driven through its window (Windows)
```

The end-to-end run starts the debug build against fake servers, so it never touches your installed app's login or settings. It does open a tab in your default browser per sign-in, which the test then takes over.

### Build installers

```bash
npm run build:desktop
```

The default Tauri build creates the platform bundle targets configured for the current OS. Windows uses `src-tauri/tauri.conf.json`; macOS additionally merges `src-tauri/tauri.macos.conf.json`.

For platform-specific builds:

```bash
npm run build:windows  # Windows NSIS installer
npm run build:mac      # macOS universal .app + .dmg, run this on macOS
```

The Windows NSIS installer is created in `src-tauri/target/release/bundle/nsis/`. The macOS universal `.dmg` and `.app` bundles are created in `src-tauri/target/universal-apple-darwin/release/bundle/`.

From a Windows machine, the fastest validation loop for macOS packaging changes is to run `npm test`, `npx tsc --noEmit`, and use the CI workflow on the feature branch so the macOS GitHub Actions runner builds the `.dmg`. Native macOS bundling must run on macOS.

## Project structure

```
src/                    React + TypeScript frontend
  components/           UI screens (Login, Recorder, Settings, StatusBar)
  hooks/                React hooks (useAuth, useRecorder, useSSE)
  i18n/                 German translations
  lib/                  Typed Tauri command wrappers
  test/                 Vitest test suite

src-tauri/              Rust backend
  src/
    capture/            Screen capture, audio recording, input hooks
    commands/           Tauri commands (auth, recording, generate, settings)
    network/            Server communication (upload, SSE, auth)
    output/             PDF generation, markdown, crash recovery
    state.rs            Shared app state
    tray.rs             System tray
```

## CI/CD

- **CI** (`ci.yml`): Runs on push to `main`, `feature/**`, pull requests, and manual dispatch. Tests, `cargo fmt --check`, clippy with `-D warnings` and the TypeScript check run every time, on both Windows and macOS; the installers do not, because a macOS bundle takes about ten minutes and most commits cannot affect packaging. Installers are built on `main`, on a manual run (`gh workflow run ci.yml --ref <branch>`), or when a commit pushed to `main` or a `feature/**` branch has `[build]` in its message -- pull-request runs and other branches ignore the marker -- and are uploaded as artifacts. An `e2e` job drives the real app through its window on Windows only: WKWebView on macOS offers no port for Playwright to attach to, so macOS-specific behaviour is checked by hand
- **Release** (`release.yml`): Runs on `v*` tags -- Windows/macOS release builds + GitHub Release with auto-updater manifest. A `server-contract` job runs first and refuses the release while production does not serve an endpoint the build depends on, so the server is always deployed before a recorder that needs it

## License

BSL-1.1
