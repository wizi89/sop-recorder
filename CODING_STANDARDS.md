# CODING_STANDARDS.md

The rules a reviewer checks a PR against. Each rule is checkable on a diff and names its
source and, where one exists, the tool that enforces it. A rule with no tool is checked by
the reviewer.

Every rule is derived from what this repo already says or enforces: `AGENTS.md`, the CI
workflow, configs, and the `Rule:` lines of `docs/FINDINGS.md`. `AGENTS.md` keeps the steering;
this file states the rules.

## General

- **G1.** This repo is public. No material from the private repo in code, commits, PR text or
  issue comments: no customer, pilot, pricing or internal infrastructure details.
  Source: `AGENTS.md:3`, `AGENTS.md:25-26`.
- **G2.** Default to the simplest solution that works. No abstractions the task did not ask for.
  Source: `AGENTS.md:4`.
- **G3.** A surprise that took real debugging time and is not plain from the code gets a
  `docs/FINDINGS.md` entry (newest at the top, `Symptom / Cause / Rule`) in the same task.
  Source: `AGENTS.md:5-6`; `docs/FINDINGS.md:6-8`.
- **G4.** A change a user can notice adds a line under `## [Unreleased]` in `CHANGELOG.md`, in the
  Keep a Changelog sections (`Added`, `Changed`, `Fixed`), written for the user.
  Source: `CHANGELOG.md:1-8`; practice in #18, #21, #23.
- **G5.** PR titles (the squash commit) use Conventional Commits: `fix:`, `feat(<area>):`,
  `docs:`, `chore:`, `chore(deps):`. Source: `git log` of `main`.
- **G6.** Line endings stay LF (CRLF only for `*.cmd`/`*.bat`); `.gitattributes` is not relaxed.
  Source and tool: `.gitattributes`.
- **G7.** Signing keys and secrets are never committed; `src-tauri/keys*` stays gitignored and
  CI reads keys only from `secrets.*`. Source: `.gitignore:38-40`; `.github/workflows/ci.yml`
  build job.
- **G8.** A file over the 5000-token cap that a change touches is split first, in its own commit
  with behaviour unchanged. The split is in scope: a reviewer never flags it as scope creep, and
  checks only that behaviour stayed the same. Hooks are never skipped (`--no-verify`) or edited to
  get a commit through. Source: `AGENTS.md` Pre-commit checks.
  Tool: `.githooks/pre-commit` (token cap, report-only for now).

## Server contract

- **C1.** A change to what the app sends to or reads from the server lands in
  `docs/server-contract.md` in the same PR, and follows the contract phase line in `AGENTS.md`
  (today: breaking allowed). Source: `docs/server-contract.md:168-176`; `AGENTS.md:12-13`.
- **C2.** When the app starts depending on a new endpoint, the `server-contract` job in
  `.github/workflows/release.yml` gets a line for it. Source: `.github/workflows/release.yml:16-17`;
  `docs/server-contract.md:174-175`.
- **C3.** What leaves the machine is listed in `docs/ARCHITECTURE.md` "What leaves the machine"; a
  change to it updates that table in the same PR. Source: `docs/ARCHITECTURE.md:39-51`.

## Frontend (React + TypeScript)

- **F1.** TypeScript `strict`, `noUnusedLocals`, `noUnusedParameters` and
  `noFallthroughCasesInSwitch` stay on. Source: `tsconfig.json:18-21`. Tool: CI TypeScript check
  (`npx tsc --noEmit`, `.github/workflows/ci.yml:73-74`).
- **F2.** UI text goes through `t()` with a key in `src/i18n/de.ts`, in German with real umlauts
  (`ä ö ü ß`, never `ae`/`oe`/`ue`/`ss`). Source: `src/i18n/index.ts`, `src/i18n/de.ts`.
- **F3.** A Tauri unlisten function is never called directly; it goes through `safeUnlisten`.
  Every `listen` cleanup has the `cancelled` guard and survives a second teardown.
  Source: `docs/findings/2026-09-and-earlier.md` 2026-09-04 (unlisten returns a promise; the error reporter
  reported itself). Tool: `src/test/safeUnlisten.test.ts` source scan.

## Rust

- **R1.** `cargo fmt` clean. Source and tool: CI Rust format (`.github/workflows/ci.yml:62-67`).
- **R2.** `cargo clippy --all-targets -- -D warnings` clean; a lint is fixed, not allowed away
  without a comment saying why. Source and tool: CI Clippy (`.github/workflows/ci.yml:62-71`).
- **R3.** Monitor geometry: never combine `Monitor::scale_factor()` with
  `Monitor::width()/height()/x()/y()`; derive the density from the capture
  (`capture_image().width() / Monitor::width()`). Source: `docs/findings/2026-09-and-earlier.md` 2026-08-29.
- **R4.** A side effect that must accompany a state change lives inside the function that makes
  the change, not beside each call. Source: `docs/findings/2026-09-and-earlier.md` 2026-09-04 (a report was written
  and no dialog opened).
- **R5.** Platform code sits behind `cfg(windows)` / `cfg(target_os = "macos")`, with its
  dependencies in the matching `[target.'cfg(...)'.dependencies]` table. Both platforms must
  build: Windows and macOS are equal targets. Source: `src-tauri/Cargo.toml`;
  `.github/workflows/ci.yml:19-28`. Tool: the CI `test` matrix.

## Tests

- **T1.** Frontend tests are vitest (`npm test`); Rust tests run with `cargo test --lib`. Both run
  on Windows and macOS. Source and tool: `.github/workflows/ci.yml:24-60`.
- **T2.** A test that reads source text normalises line endings first
  (`include_str!(...).replace("\r\n", "\n")`). Source: `docs/findings/2026-09-and-earlier.md` 2026-09-04
  (source-scanning test failed on Windows).
- **T3.** A tool that walks the tree (test runner, linter, formatter) excludes
  `.claude/worktrees/**` in its own config. Source: `docs/FINDINGS.md` 2026-11-14;
  `vitest.config.ts`.
- **T4.** An e2e test waits for the UI (a visible button) before changing window state, and checks
  the state still holds a second later. e2e runs only in CI on Windows. Source:
  `docs/FINDINGS.md` 2026-10-24; `AGENTS.md:43`.
- **T5.** Behaviour unit tests cannot reach (real OS input, capture, permissions) is checked by
  hand: a fixture in `docs/manual-tests/` where one fits, and the owner's hands-on check named in
  the PR body. Source: `docs/manual-tests/README.md:1-4`; `AGENTS.md:55-57`; `README.md` CI/CD
  (macOS behaviour is checked by hand).

## Not yet a rule

Candidates only. Review's verdict lists the "Rules this review wanted and could not find" here;
only the owner promotes one into a rule above.

- New behaviour comes with tests at the highest seam that exercises it (a rule in the server
  repo, not yet stated here).
