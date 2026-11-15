# AGENTS.md

- **This repo is public.** Material from the private repo stays out of commits, PR text and code.
- Default to the simplest solution that works, no unrequested abstractions.
- Surprises go to `docs/FINDINGS.md`; drain it with `drain-findings`. For a symptom, grep FINDINGS
  and the Wiki index first.
- Modules, the life of a recording, and what leaves the machine: `docs/ARCHITECTURE.md`.
- Server contract (endpoints, fields and SSE events the recorder uses): `docs/server-contract.md`.
- Contract changes: breaking allowed. (Until go-live; then this line becomes "Contract changes: additive only".)
- The default branch here is `main`; the server repo's is `master`.
- Agent merges are allowed: an agent merges its own PR (squash) when every check passed and nothing had to be guessed; otherwise it labels the issue `needs-owner`.

## Pre-commit checks

The `test` job in `.github/workflows/ci.yml`, grouped by what changed:

- Always: `npm ci`, `npm test`, `npx tsc --noEmit`.
- When `src-tauri/` changed, also run in `src-tauri/`: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`.
- e2e (`npm run e2e`) runs only in CI on Windows; don't run it locally.

A check the session can't run (for example, Rust system libraries missing in a cloud session)
counts as not passed: stop and label the issue `needs-owner`.
