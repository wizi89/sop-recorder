# AGENTS.md

- **This repo is public.** Material from the private repo stays out of commits, PR text and code.
- Default to the simplest solution that works, no unrequested abstractions.
- Surprises go to `docs/FINDINGS.md`; drain it with `drain-findings`. For a symptom, grep FINDINGS
  and the Wiki index first.
- Modules, the life of a recording, and what leaves the machine: `docs/ARCHITECTURE.md`.
- Roadmap and current status of the whole product: `../9_sop-sorcery/docs/ROADMAP.md` and
  `project-status.md` next to it.
- The default branch here is `main`; the server repo's is `master`.
- The code rules a PR is reviewed against: `CODING_STANDARDS.md`.
- Server contract: `docs/server-contract.md`. Contract changes: breaking allowed. (Until go-live;
  then this line becomes "Contract changes: additive only", in both repos.)

## Agents never merge

A merge to `main` builds the installers, and a `v*` tag ships them to every user through the
auto-updater. An agent never merges a PR and never enables auto-merge: when CI is green, it sets
the label `owner-merge` on the PR and stops; the owner merges. When checks fail or it had to
guess, it labels the issue `needs-owner`.

Issues the factory works live in this repo and are written for a public audience: the problem
and the wanted behaviour, nothing from the private repo (no customer, pilot or pricing details).
Labels: `agent:explore`, `agent:implement`, `agent:review` (PR), `agent:in-progress`,
`agent:blocked`, `needs-owner`, `owner-merge`, `one-way-door`.

One-way door paths (Implement adds `one-way-door` and the checklist when a diff touches one):
`.github/`, `src-tauri/tauri.conf.json`, `src-tauri/tauri.macos.conf.json`,
`src-tauri/tauri.signing.conf.json`, `src-tauri/Entitlements.plist`, `src-tauri/Info.plist`,
`src-tauri/capabilities/`, `docs/server-contract.md`.

## Pre-commit checks

The `test` job in `.github/workflows/ci.yml`, grouped by what changed:

- Always: `npm ci`, `npm test`, `npx tsc --noEmit`.
- When `src-tauri/` changed, also run in `src-tauri/`: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`.
- e2e (`npm run e2e`) runs only in CI on Windows; don't run it locally.

In a cloud session (`CLAUDE_CODE_REMOTE=true`) the image has no Tauri system libraries, and a
Linux build would check neither the Windows nor the macOS code paths. There:

- Run `npm ci`, `npm test`, `npx tsc --noEmit` in the session, and `cargo fmt --check` in
  `src-tauri/` when `cargo` is installed (it compiles nothing).
- `cargo clippy` and `cargo test --lib` are **CI-only**: open the draft PR, then wait for CI's
  `ci` check on the head commit (`gh api repos/wizi89/sop-recorder/commits/<sha>/check-runs`).
  It gates `test (windows-latest)`, `test (macos-latest)` and `e2e`, which a Markdown-only
  change skips. Its result is the check result; a red `ci` counts as a failed round.

The factory can't run the real app. A change to capture, input hooks, windows, permissions or
audio needs the owner's hands-on check on Windows, and on macOS when macOS code changed, before
the merge. Say so in the PR body.
