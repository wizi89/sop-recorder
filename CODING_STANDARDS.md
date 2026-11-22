# CODING_STANDARDS.md

The rules a reviewer checks a PR against. Only judgement calls live here: a rule a tool can
check is a check instead (listed at the end), and process rules live in `AGENTS.md`. Rule IDs
are stable; a deleted rule leaves a gap.

## General

- **G1.** This repo is public. No material from the private repo in code, commits, PR text or
  issue comments: no customer, pilot, pricing or internal infrastructure details.
- **G2.** Default to the simplest solution that works. No abstractions the task did not ask for.
- **G8.** A split of a file over the 5000-token cap, in its own commit before the change, is in
  scope: never flag it as scope creep, check only that behaviour stayed the same.
- **G9.** Names in code, UI text and docs use the `GLOSSARY.md` terms. The CI job `glossary`
  checks the `_Avoid_` entries without a qualifier; the reviewer checks the qualified ones.
- **G10.** A product *why* (a decision that is hard to reverse, surprising without context, and
  the result of a real trade-off) goes in an ADR in `docs/adr/`, not in a code comment; the comment
  may point to the ADR. Reversing a decision means a new ADR that says which one it replaces.

## Server contract

- **C1.** A change to what the app sends to or reads from the server lands in
  `docs/server-contract.md` in the same PR, and follows the contract phase line in `AGENTS.md`.
- **C2.** When the app starts depending on a new endpoint, the `server-contract` job in
  `.github/workflows/release.yml` gets a line for it.
- **C3.** A change to what leaves the machine updates the "What leaves the machine" table in
  `docs/ARCHITECTURE.md` in the same PR.

## Frontend (React + TypeScript)

- **F2.** UI text goes through `t()` with a key in `src/i18n/de.ts`, in German with real umlauts
  (`ä ö ü ß`, never `ae`/`oe`/`ue`/`ss`).
- **F3.** Every `listen` cleanup has the `cancelled` guard and survives a second teardown. (A
  direct unlisten call is the Semgrep rule `direct-unlisten-call`.)
  Source: `docs/findings/2026-09-and-earlier.md` 2026-09-04.

## Rust

- **R3.** Monitor geometry: never combine `Monitor::scale_factor()` with
  `Monitor::width()/height()/x()/y()`; derive the density from the capture
  (`capture_image().width() / Monitor::width()`). Source: `docs/findings/2026-09-and-earlier.md`
  2026-08-29.
- **R4.** A side effect that must accompany a state change lives inside the function that makes
  the change, not beside each call. Source: `docs/findings/2026-09-and-earlier.md` 2026-09-04.

## Tests

- **T2.** A test that reads source text normalises line endings first
  (`include_str!(...).replace("\r\n", "\n")`). Source: `docs/findings/2026-09-and-earlier.md`
  2026-09-04.
- **T3.** A tool that walks the tree (test runner, linter, formatter) excludes
  `.claude/worktrees/**` in its own config. Source: `docs/FINDINGS.md` 2026-11-14.
- **T4.** An e2e test waits for the UI (a visible button) before changing window state, and checks
  the state still holds a second later. Source: `docs/FINDINGS.md` 2026-10-24.

## Checks that replaced rules

Skip these in review; CI fails on them.

| Former rule | Check |
|---|---|
| F1 TypeScript strict flags | `src/test/tsconfig.test.ts`; `npx tsc --noEmit` |
| F3 direct unlisten call | `.semgrep/unlisten.yml`, run by `python scripts/check_semgrep.py` (CI job `semgrep`) |
| G6 LF line endings | `.gitattributes` |
| G7 no committed secrets | `.gitignore`; GitHub secret scanning with push protection |
| R1 `cargo fmt` | CI `cargo fmt --check` |
| R2 clippy clean, an allowed lint says why | CI `cargo clippy --all-targets -- -D warnings`; `allow_attributes_without_reason` in `src-tauri/Cargo.toml` |
| R5 both platforms build | CI `test` matrix (Windows and macOS) |
| One-way Door paths (`AGENTS.md`) | `.github/workflows/one-way-door.yml` labels the PR and appends the checklist |

## Not yet a rule

Candidates only. Review's verdict lists the "Rules this review wanted and could not find" here;
only the owner promotes one into a rule above.

- New behaviour comes with tests at the highest seam that exercises it (a rule in the server
  repo, not yet stated here).
