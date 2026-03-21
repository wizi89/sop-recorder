# SOP Recorder

Desktop app that captures screen clicks and audio narration, then generates polished SOP (Standard Operating Procedure) documents using AI.

Built with [Tauri v2](https://tauri.app/) (Rust + React + TypeScript).

## Download

Download the latest installer from [GitHub Releases](https://github.com/wizi89/sop-recorder/releases/latest). Run the `.exe` -- no dependencies required, no admin rights needed.

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
- Per-user NSIS installer (no admin rights)

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) 22+
- [Rust](https://rustup.rs/) stable
- Windows 10/11

### Setup

```bash
npm install
```

### Run in dev mode

```bash
npx tauri dev
```

### Run tests

```bash
npm test                          # Frontend tests (vitest)
cd src-tauri && cargo test --lib  # Rust tests
```

### Build installer

```bash
npx tauri build
```

The NSIS installer is created in `src-tauri/target/release/bundle/nsis/`.

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

- **CI** (`ci.yml`): Runs on push to `main` -- tests + build, uploads installer artifact
- **Release** (`release.yml`): Runs on `v*` tags -- tests + build + GitHub Release with auto-updater manifest

## License

BSL-1.1
