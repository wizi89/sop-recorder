# SOP Recorder

Desktop app that captures screen clicks and audio narration, then generates polished SOP (Standard Operating Procedure) documents using AI.

Built with [Tauri v2](https://tauri.app/) (Rust + React + TypeScript).

## Download

Download the latest installer from [GitHub Releases](https://github.com/wizi89/sop-recorder/releases/latest). On Windows, run the `.exe` -- no dependencies required, no admin rights needed. On macOS, download the `.dmg`, open it, and drag the app into Applications.

## How it works

1. Click **Aufnahme starten** (Start Recording)
2. Perform your workflow -- every mouse click is captured as a screenshot
3. Narrate what you're doing -- audio is recorded simultaneously
4. Click **Aufnahme stoppen** (Stop Recording)
5. The app uploads screenshots + audio to the server, which uses AI (Whisper + GPT-4o) to generate a step-by-step guide
6. A markdown file and PDF are saved locally

## Features

- Screen capture on mouse click with click position overlay
- Audio recording (16kHz mono WAV, resampled from device native rate)
- Server-side AI pipeline with SSE progress streaming
- Local PDF + markdown generation
- Supabase authentication (via server proxy)
- Crash recovery (pending.json retry)
- Auto-updater via GitHub Releases
- System tray integration
- German UI (i18n)
- Per-user NSIS installer on Windows (no admin rights)
- macOS `.dmg` and `.app` bundles

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) 22+
- [Rust](https://rustup.rs/) stable
- Windows 10/11 for Windows installers
- macOS 14+ with Xcode Command Line Tools for macOS bundles

### Setup

```bash
npm install
```

### Run in dev mode

```bash
npx tauri dev
```

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
```

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

- **CI** (`ci.yml`): Runs on push to `main`, `feature/**`, pull requests, and manual dispatch. Tests run every time; the installers do not, because a macOS bundle takes about ten minutes and most commits cannot affect packaging. Installers are built on `main`, on a manual run, or on any commit whose message contains `[build]`, and are uploaded as artifacts
- **Release** (`release.yml`): Runs on `v*` tags -- Windows/macOS release builds + GitHub Release with auto-updater manifest

## License

BSL-1.1
