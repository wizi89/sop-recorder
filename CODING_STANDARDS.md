# CODING_STANDARDS.md

The rules a reviewer checks a PR against. Only judgement calls live here: a rule a tool can
check is a check instead (listed at the end), and process rules live in `AGENTS.md`. Rule IDs
are stable; a deleted rule leaves a gap.

## General

- **G1.** This repo is public. No material from the private repo in code, commits, PR text or
  issue comments: no customer, pilot, pricing or internal infrastructure details.
- **G2.** Default to the simplest solution that works. No abstractions the task did not ask for.
- **G3.** A surprise that took real debugging time and is not plain from the code gets a
  `docs/FINDINGS.md` entry (newest at the top, `Symptom / Cause / Rule`) in the same task.
- **G7.** CI reads signing keys and secrets only from `secrets.*`, never from a file in the repo
  or a value in a workflow. (A committed secret is caught by the checks below.)
- **G8.** A split of a file over the 5000-token cap, in its own commit before the change, is in
  scope: never flag it as scope creep, check only that behaviour stayed the same.
- **G9.** Names in code, UI text and docs use the `GLOSSARY.md` terms. The CI job `glossary`
  checks the `_Avoid_` entries without a qualifier; the reviewer checks the qualified ones.
- **G10.** A product *why* (a decision that is hard to reverse, surprising without context, and
  the result of a real trade-off) goes in an ADR in `docs/adr/`, not in a code comment; the comment
  may point to the ADR. Reversing a decision means a new ADR that says which one it replaces.
- **G11.** A new check ships with a bad fixture it must flag, and fails when it checked zero
  files: a check that scanned nothing proves nothing (`scripts/check_semgrep.py` shows how).

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

- **R2.** A clippy lint is fixed, not allowed away. An `#[allow]` is the last resort, and its
  `reason` says why the fix would be worse, not what the lint is. (That a reason exists is a
  check, below.)
- **R3.** Monitor geometry: never combine `Monitor::scale_factor()` with
  `Monitor::width()/height()/x()/y()`; derive the density from the capture
  (`capture_image().width() / Monitor::width()`). Source: `docs/findings/2026-09-and-earlier.md`
  2026-08-29.
- **R4.** A side effect that must accompany a state change lives inside the function that makes
  the change, not beside each call. Source: `docs/findings/2026-09-and-earlier.md` 2026-09-04.
- **R5.** A platform-only dependency goes in the matching `[target.'cfg(...)'.dependencies]`
  table of `src-tauri/Cargo.toml`, not in `[dependencies]`. (That both platforms build is a
  check, below.)

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
| R6 module boundary: only `src-tauri/src/network/` talks HTTP (`reqwest`, `reqwest_eventsource`), so what leaves the machine is one directory to read | `.semgrep/network-boundary.yml`, same script and job; prints `boundaries: checked N files` |
| G6 LF line endings | `.gitattributes` |
| G7 no committed secrets (the `secrets.*` half stays a rule) | `.gitignore`; GitHub secret scanning with push protection |
| R1 `cargo fmt` | CI `cargo fmt --check` |
| R2 clippy clean, an allowed lint says why | CI `cargo clippy --all-targets -- -D warnings`; `allow_attributes_without_reason` in `src-tauri/Cargo.toml` |
| R5 both platforms build | CI `test` matrix (Windows and macOS) |
| One-way Door paths (`AGENTS.md`) | `.github/workflows/one-way-door.yml` labels the PR and appends the checklist |

## Not yet a rule

Candidates only. Review's verdict lists the "Rules this review wanted and could not find" here;
only the owner promotes one into a rule above.

- New behaviour comes with tests at the highest seam that exercises it (a rule in the server
  repo, not yet stated here).

Wanted in two or more Reviews (2026-11-15 to 2026-11-23):

- The Conventional Commit types include `ci:`, `test:` and `fix(ci):`, which `main` already uses
  ([#43](https://github.com/wizi89/sop-recorder/issues/43#issuecomment-6991877837),
  [#54](https://github.com/wizi89/sop-recorder/issues/54#issuecomment-7062712340),
  [#87](https://github.com/wizi89/sop-recorder/issues/87#issuecomment-7086864483)).
- A one-way door checklist for client doors (`docs/server-contract.md`, what leaves the machine);
  the current checklist is the server template
  ([#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7063187576)).
- Whether append-only logs (`CHANGELOG.md`, `docs/FINDINGS.md`) are exempt from the token cap
  ([#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7062991435)).
- Workflows pass actionlint and keep `permissions:` least-privilege
  ([#46](https://github.com/wizi89/sop-recorder/issues/46#issuecomment-7013475487),
  [#54](https://github.com/wizi89/sop-recorder/issues/54#issuecomment-7062712340)).

Wanted once:

- A FINDINGS search covers `docs/findings/` too; the poisoned-mutex source guard is listed (#26).
- A PR that shifts `AGENTS.md` lines updates the `AGENTS.md:N` references (#46).
- A Markdown line width; a written label-state contract between the workflows and the Deps
  Station (#54).
- A doc comment width; `docs/PRIVACY.md` kept in step with `docs/ARCHITECTURE.md` (#63).
- Colours come from theme tokens, not hex (#76).
- A test's name claims only what it asserts (#87).
- A unit test lives next to the function it covers; user-facing CHANGELOG lines do not name
  internal files (#92).
