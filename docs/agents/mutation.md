# Mutation testing

A mutant is a small change to the code, such as `<` turned into `<=` or a
function made to return a default. A test that fails on it kills it. A
survivor is a change no test noticed: code the tests run but do not check.

This is information for the author, not a gate. A survivor never fails a run.

## What gets mutated

`scripts/mutation_diff.sh [base] [report]` (default `origin/main` and
`reports/mutation/report.md`) mutates what the branch changed:

- **Rust:** cargo-mutants on the lines the diff touches in `src-tauri/src`,
  tested with `cargo test --lib`. It mutates in place, so commit first; it
  restores each file, but an interrupted run can leave one behind, which
  `git diff` shows.
- **TypeScript:** Stryker (`stryker.config.json`, vitest runner) on the `src/`
  modules the branch added or changed, not tests.

## Where it runs

- **The author's session, at the end of the work:** run the script. It needs
  `cargo install --locked cargo-mutants` once; Stryker comes with `npm ci`.
- **CI:** the job `mutation` runs it on every PR push that touches code, on
  Linux. It writes the report to the job summary and uploads it as the
  artifact `mutation-report`. It is not part of `ci`, so it never blocks a
  merge; a newer push cancels a run still going.

## Reading the report

The report counts each tool's results and lists up to 30 survivors per
language: file, line and the mutation.

For each survivor:

1. **Kill it:** write the test that would fail on that change. It is the same
   red-green step as `/tdd`: the test must fail on the mutant. Then rerun the
   script and check the mutant is now caught.
2. **Or say why not, in the PR body:** the mutant is equivalent (the behaviour
   cannot change), or the code is logging or a message no user depends on.

Fetch a CI report with `gh run download <run-id> -n mutation-report`; the run
id is in `gh run list --workflow ci.yml --branch <branch>`.
