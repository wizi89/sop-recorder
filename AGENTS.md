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
- A change a user can notice adds a line under `## [Unreleased]` in `CHANGELOG.md`, in the Keep a
  Changelog sections (`Added`, `Changed`, `Fixed`), written for the user.
- PR titles (the squash commit) use Conventional Commits: `fix:`, `feat(<area>):`, `docs:`,
  `chore:`, `chore(deps):`.
- Prototype code lives only on `prototype/*` branches and never reaches `main`. What a prototype
  taught goes into the issue, an ADR or `GLOSSARY.md`; the code does not.
- Server contract: `docs/server-contract.md`. Contract changes: breaking allowed. (Until go-live;
  then this line becomes "Contract changes: additive only", in both repos.)

## Domain language and planning

- Domain terms: `GLOSSARY.md`. Decisions that are hard to reverse: `docs/adr/`. The owner owns
  both (`.github/CODEOWNERS`); a change to either is pointed out in the PR body.
- Plan by size. A tiny diff with no open question is done in one shot. Anything else starts
  with `/grill-with-docs`, which settles the open questions with the owner and records terms and
  ADRs as it goes. `/wayfinder` only when the effort is too foggy for one grilling session.
  A multi-PR effort continues with `/to-spec`, then `/to-tickets`; labels, blocking edges and how
  tickets ship: `docs/agents/issue-tracker.md`.
- Before publishing an issue (a spec, a ticket), write the draft (title, blank line, body) to a
  file and run `python3 scripts/check_glossary_terms.py --text <file>`; fix every hit. Prose
  only: `code spans` may quote an old name. The `Issue glossary` workflow runs the same check on
  every opened or edited issue and comments on a hit.
- **Renames land before `/to-spec`.** A glossary change and the code renames it causes merge to
  `main` in their own PR first, so the spec and its tickets use only the new names.
- **A dense ADR gets a second pass** before its PR leaves draft. Dense means it settles more than
  one decision, or its decision is a one-way door. A fresh-context subagent gets only the ADR,
  `GLOSSARY.md` and the code the ADR names, and lists contradictions, alternatives it doesn't
  reject, consequences it leaves out, and terms the glossary lacks. Fix or answer each point; the
  PR body says `Second ADR pass: N points, all fixed or answered`.
- A station has no owner to grill: an open question becomes `needs-owner`.

## Agents never merge

A merge to `main` builds the installers, and a `v*` tag ships them to every user through the
auto-updater. An agent never merges a PR and never enables auto-merge: when CI is green, it sets
the label `owner-merge` on the PR and stops; the owner merges. When checks fail or it had to
guess, it labels the issue `needs-owner`.

A step only a person can take (a secret, a signing certificate, a store or dashboard setting)
goes to `needs-owner` with the reason, plus a `/wizard` script that walks the owner through it.
The wizard asks for every secret itself, so the value never passes through the session.

Exception (owner decision 2026-11-18): `dependabot-lane.yml` enables auto-merge on Dependabot PRs
whose bumps are all patches. Agents still never merge or enable auto-merge. A `github_actions`
bump never takes that lane or goes to the Deps Station: it gets `owner-merge` (owner decision
2026-11-21).

Issues the factory works live in this repo and are written for a public audience: the problem
and the wanted behaviour, nothing from the private repo (no customer, pilot or pricing details).
Labels: `agent:explore`, `agent:implement`, `agent:review` (PR), `agent:in-progress`,
`agent:blocked`, `agent:deps`, `needs-owner`, `owner-merge`, `one-way-door`.

One-way door paths. `.github/workflows/one-way-door.yml` adds the `one-way-door` label and a
checklist when a PR touches one, and stays red until every box is ticked; keep this list and the
workflow's in step.

- Release: `.github/workflows/release.yml`, `src-tauri/tauri.conf.json`,
  `src-tauri/tauri.macos.conf.json`, `src-tauri/tauri.signing.conf.json`, `src-tauri/Info.plist`.
- Permissions: `src-tauri/Entitlements.plist`, `src-tauri/capabilities/`.
- Contract: `docs/server-contract.md`.
- What leaves the machine: `src-tauri/src/network/upload.rs`, `src-tauri/src/network/reports.rs`,
  `src-tauri/src/network/auth.rs`, `src-tauri/src/error_reports.rs` and `error_reports/`,
  `src-tauri/src/commands/error_reports.rs`, `src-tauri/src/commands/generate/redact.rs`,
  `src-tauri/src/output/redactions.rs`, `src-tauri/src/capture/password_check.rs` and
  `password_check/`, `src-tauri/src/capture/window_area.rs` and `window_area/`.
- Dependabot: `.github/workflows/dependabot-lane.yml`, `.github/workflows/dependabot-red.yml`.
- Guardrails (a weakened check stays green): `.github/workflows/one-way-door.yml`,
  `.github/workflows/ci.yml`, `.github/workflows/codeql.yml`, `.githooks/`,
  `.claude/hooks/enable-githooks.sh`, `.semgrep/`, `.gitattributes`, `biome.json`, `tsconfig.json`,
  `tsconfig.node.json`, `src/test/tsconfig.test.ts`, `scripts/check_*`, `scripts/test_check_*`,
  `scripts/glossary_*`. The box asks "Does this weaken a check?" and is the owner's: no session
  ticks it. A Dependabot PR gets none while only Dependabot pushed to it.

## Pre-commit checks

The git hooks in `.githooks/` turn on by themselves when `core.hooksPath` is unset: a SessionStart hook runs
`git config core.hooksPath .githooks`. The pre-commit hook blocks the commit when a staged code
file is over the 5000-token cap (docs and data are not capped; generated Tauri files are exempt),
or a staged file fails `rustfmt --check` or Biome's format check (Biome needs `npm ci`). It runs
in about 2 seconds; tsc, clippy and the tests stay in CI. Never skip a hook (`--no-verify`) or
edit one to get a commit through.
`sh .githooks/selftest.sh` proves the pre-commit hook still catches what it should.

**Editing a large file means splitting it.** When a file you change is over the token cap,
split it first, in its own commit with behaviour unchanged. That split is in scope for your task.

The `test` job in `.github/workflows/ci.yml`, grouped by what changed:

- Always: `npm ci`, `npm test`, `npx tsc --noEmit`, `npx biome ci .` (formatting; `npx biome format --write .`
  fixes it).
- Always, in its own CI job `semgrep`: `python scripts/check_semgrep.py`, the custom Semgrep rules
  in `.semgrep/` (needs `semgrep` or `uvx`). It also proves each rule still flags its bad fixture.
  One of them is the module boundary: no HTTP client outside `src-tauri/src/network/` (R6).
- Always, in its own CI job `glossary`: `python3 scripts/check_glossary_terms.py` (a second; run
  it before pushing, a miss costs a CI round).
- Always, in its own CI job `actionlint`: actionlint with shellcheck on `.github/workflows/`. Don't
  install it mid-session; read the job's result.
- When `src-tauri/` changed, also run in `src-tauri/`: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`.
- e2e (`npm run e2e`) runs only in CI on Windows; don't run it locally.

In a cloud session (`CLAUDE_CODE_REMOTE=true`), the SessionStart hook `.claude/hooks/cloud-setup.sh`
installs the Tauri system libraries and runs `npm ci`. Its last line says whether that worked. There:

- Run everything in the `test` list above, `cargo clippy` and `cargo test --lib` included. CI's
  `test (ubuntu-latest)` leg runs the same hook and checks, so they keep working.
- A Linux build checks neither the Windows nor the macOS code paths, and e2e needs Windows. So CI's
  `ci` check is still the result: open the draft PR, then wait for `ci` on the head commit
  (`gh api repos/wizi89/sop-recorder/commits/<sha>/check-runs`). It gates `test (windows-latest)`,
  `test (macos-latest)` and `e2e`, which a Markdown-only change skips. A red `ci` counts as a
  failed round.

Logs, error reports and the CI artifacts, including the e2e screens a UI PR links:
`docs/agents/data-sources.md`.

At the end of work on code, run `bash scripts/mutation_diff.sh`: cargo-mutants on the changed Rust
lines, Stryker on the changed TypeScript modules. It reports the survivors as information, not a
gate; CI's `mutation` job runs it on every PR push. `docs/agents/mutation.md` says how to read it.

The factory can't run the real app. A change to capture, input hooks, windows, permissions or
audio needs the owner's hands-on check on Windows, and on macOS when macOS code changed, before
the merge. Say so in the PR body.
