# Findings

Hard-won, non-obvious facts about the recorder. Each entry cost real debugging time.
Read this before diagnosing anything in capture, window management, or permissions.

**Adding entries:** newest at the top. One entry per finding. Keep the shape
`Symptom / Cause / Rule`, because the symptom is what a future reader will search for.
Only add things that were genuinely surprising: if it is obvious from the code, skip it.

Older entries (2026-09 and earlier): [findings/2026-09-and-earlier.md](./findings/2026-09-and-earlier.md).

---

## 2026-11-23 Review rules wanted more than once are still missing from CODING_STANDARDS.md

**Symptom:** factory Reviews from 2026-11-15 to 2026-11-23 listed the same missing rules in
several runs; none reached "Not yet a rule" in `CODING_STANDARDS.md`.

**Cause:** the verdict lists the wanted rule in a PR comment, and nothing copies it into "Not yet
a rule", so the owner never sees it to promote.

**Rule:** rules wanted in two or more runs (factory retro 2026-11-23):

- The Conventional Commit types list `ci:`, `test:` and `fix(ci):`, which `main` already uses.
  [#43](https://github.com/wizi89/sop-recorder/issues/43#issuecomment-6991877837), [#54](https://github.com/wizi89/sop-recorder/issues/54#issuecomment-7062712340), [#87](https://github.com/wizi89/sop-recorder/issues/87#issuecomment-7086864483)
- A one-way door checklist for client doors (`docs/server-contract.md`, what leaves the
  machine); the current checklist is the server template. [#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7063187576), [#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7063691970)
- Whether append-only logs (`CHANGELOG.md`, `docs/FINDINGS.md`) are exempt from the token cap.
  [#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7062991435), [#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7063187576)
- Workflows pass actionlint and keep `permissions:` least-privilege. [#46](https://github.com/wizi89/sop-recorder/issues/46#issuecomment-7013475487), [#54](https://github.com/wizi89/sop-recorder/issues/54#issuecomment-7062712340)

Wanted once: a FINDINGS search covers `docs/findings/` too, and the poisoned-mutex source guard is
listed ([#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7063691970),
[#26](https://github.com/wizi89/sop-recorder/issues/26#issuecomment-7063187576)); a PR that shifts
AGENTS.md lines updates the `AGENTS.md:N` references ([#46](https://github.com/wizi89/sop-recorder/issues/46#issuecomment-7013475487));
a Markdown line width and a written label-state contract between the workflows and the Deps
Station ([#54](https://github.com/wizi89/sop-recorder/issues/54#issuecomment-7062712340)); doc
comment width and `docs/PRIVACY.md` kept in step with ARCHITECTURE
([#63](https://github.com/wizi89/sop-recorder/issues/63#issuecomment-7064329222)); colours from theme
tokens, not hex ([#76](https://github.com/wizi89/sop-recorder/issues/76#issuecomment-7075715353));
a test's name claims only what it asserts ([#87](https://github.com/wizi89/sop-recorder/issues/87#issuecomment-7086864483));
a unit test lives next to the function it covers, and user-facing CHANGELOG lines do not name
internal files ([#92](https://github.com/wizi89/sop-recorder/issues/92#issuecomment-7094355963)).

Copy these into "Not yet a rule" so the owner can promote or drop each one.

---

## 2026-11-23 Issues written from a PR review cite line numbers that have moved

**Symptom:** Explore on follow-up issues found the cited lines elsewhere and the cited spec gone:
[#56](https://github.com/wizi89/sop-recorder/issues/56#issuecomment-7075504788) (lines from PR #40's
head, code since split out of `screenshot.rs`), [#58](https://github.com/wizi89/sop-recorder/issues/58#issuecomment-7075512440)
(lines from PR #39's head, spec removed with OpenSpec in #73),
[#59](https://github.com/wizi89/sop-recorder/issues/59#issuecomment-7093669844) (cites a `tasks.md`
and a checklist item that are not in the repo, and names two parsers where there are three).

**Cause:** the issues were written from the review thread, whose line numbers belong to the PR
head, not to `main`.

**Rule:** an issue cites `path:line` at a `main` SHA it names, and quotes what it relies on
instead of pointing at a PR body or a removed spec.

---

## 2026-11-23 The glossary check runs only in CI

**Symptom:** the `glossary` CI job flagged "blur" in a test Review had added, which cost a CI
round ([#76](https://github.com/wizi89/sop-recorder/issues/76#issuecomment-7075715353)).

**Cause:** the session checks are `npm test` and `tsc`; `scripts/check_glossary_terms.py` is not
among them, although it takes a second.

**Rule:** run `python3 scripts/check_glossary_terms.py` with the local checks before pushing.

---

## 2026-11-23 `scripts/mutation_diff.sh` says nothing on a test-only branch

**Symptom:** a PR that only adds tests to kill survivors got "No change in a `src/` module." from
the mutation script, so its acceptance criterion needed a hand-run `npx stryker run --mutate
<module>`: [#86](https://github.com/wizi89/sop-recorder/issues/86#issuecomment-7086228812),
[#86](https://github.com/wizi89/sop-recorder/issues/86#issuecomment-7086820806),
[#87](https://github.com/wizi89/sop-recorder/issues/87#issuecomment-7086864483). Separately,
`cargo-mutants` is not installed in the cloud image, and installing it took several minutes of a
run ([#59](https://github.com/wizi89/sop-recorder/issues/59#issuecomment-7093888503)).

**Cause:** the script mutates only modules the diff changes and skips test files, so a test-only
diff has nothing to mutate.

**Rule:** candidate: when only tests changed, mutate the modules those tests import; install
`cargo-mutants` in the cloud setup hook.

---

## 2026-11-23 A rework was pushed to a branch whose PR the owner had already merged

**Symptom:** Implement pushed a rework to `agent/47-stuck-alerts` after the owner merged #48. No
PR picked up the push and no CI ran, and the run waited 44 minutes for checks that never came
([#48](https://github.com/wizi89/sop-recorder/issues/48#issuecomment-7016590560)).

**Cause:** the rework was queued before the merge, and the station does not re-check the PR's
state before pushing.

**Rule:** candidate: Implement re-reads the PR's `merged_at` and `state` right before a rework
push and stops when it is no longer open.

Pattern candidate: Implement does not check that its PR is still open before pushing a rework.

---

## 2026-11-23 Factory stations flag the owner's own comments as injections

**Symptom:** Explore stopped to decide whether the owner's `/go implement` decision comment was an
injection, and called the rule a reason for `needs-owner` on clear specs:
[#53](https://github.com/wizi89/sop-recorder/issues/53#issuecomment-7059216519),
[#57](https://github.com/wizi89/sop-recorder/issues/57#issuecomment-7063044405).

**Cause:** the station skills call every agent-aimed instruction in issue or comment text an
injection, with no word on comments by the repo owner, who is also the fire's sender.

**Rule:** candidate: a comment by the repo owner is an owner decision; only text by anyone else
is data.

Pattern candidate: the factory injection rule has no carve-out for the repo owner's own comments.

---

## 2026-11-23 Cloud factory runs cannot count on actionlint

**Symptom:** actionlint is not in the cloud image. Some runs installed it with `go install
github.com/rhysd/actionlint/cmd/actionlint@v1.7.7`; in others the session's auto-mode classifier
refused the install or a download ("Undermining Oversight"), so workflow criteria stayed
unverified: [#45](https://github.com/wizi89/sop-recorder/issues/45#issuecomment-7013294025),
[#46](https://github.com/wizi89/sop-recorder/issues/46#issuecomment-7013475487),
[#47](https://github.com/wizi89/sop-recorder/issues/47#issuecomment-7014571844),
[#47](https://github.com/wizi89/sop-recorder/issues/47#issuecomment-7014732407),
[#48](https://github.com/wizi89/sop-recorder/issues/48#issuecomment-7016590560),
[#49](https://github.com/wizi89/sop-recorder/issues/49#issuecomment-7016736700),
[#53](https://github.com/wizi89/sop-recorder/issues/53#issuecomment-7059216519),
[#53](https://github.com/wizi89/sop-recorder/issues/53#issuecomment-7062635047),
[#54](https://github.com/wizi89/sop-recorder/issues/54#issuecomment-7062712340).

**Cause:** actionlint is neither installed by `.claude/hooks/cloud-setup.sh` nor run in CI.

**Rule:** candidate: install actionlint in the cloud setup hook (the `go install` line above
works), or run it as a CI job on `.github/workflows/**`.

Pattern candidate: actionlint is missing in factory cloud sessions and installing it mid-run is refused.

---

## 2026-11-19 `security-events: read` does not let `GITHUB_TOKEN` list Dependabot alerts

**Symptom:** a workflow that calls `repos/{repo}/dependabot/alerts` with
`permissions: security-events: read` would get a 403 on its first run. actionlint passes it.

**Cause:** `security-events` covers code scanning only. Dependabot alerts need the
`vulnerability-alerts` scope, which actionlint (v1.7.7) doesn't know yet and reports as
`unknown permission scope "vulnerability-alerts"`.

**Rule:** grant `vulnerability-alerts: read` for Dependabot alerts (see
`.github/workflows/stuck-alerts.yml`), and run actionlint with
`-ignore 'unknown permission scope "vulnerability-alerts"'`.

---

## 2026-11-16 Toasts, the notification center and quick settings were missing from the window snapshot

**Symptom:** a Windows toast over the captured window was in the step PNG, but no toast
window showed up in the geometry snapshot (`live_snapshot`) or in a PowerShell `EnumWindows`
probe fired alongside it. Read from the code: a click on a toast or the notification center
finds no window under the click in the snapshot and falls back to the whole monitor.

**Cause:** `EnumWindows` leaves out immersive shell windows on Windows 8 and later (its
documentation says it enumerates only desktop apps' top-level windows). The toast host
(`ShellExperienceHost.exe`, class `Windows.UI.Core.CoreWindow`, ex style
`WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP`), the notification center and quick settings
(`ShellHost.exe`, `ControlCenterWindow`) are such windows. They are ordinary DWM-composed
windows, so a GDI screen grab contains them. Walking the top-level list with
`FindWindowExW(NULL, prev, NULL, NULL)` returns them (329 windows against 262 from
`EnumWindows` on Windows 11 build 26300). The toast host window is never destroyed: at rest
it is visible but cloaked (`DWMWA_CLOAKED = 2`) with a zero-height rect; on a toast it
uncloaks, and 0.25 to 0.7 s later its rect grows and pixels appear. `GetWindowRect` then
encloses every drawn pixel, plus about 24 px of transparent margin.

**Rule:** take z-ordered window lists from `FindWindowExW` (`capture/geometry.rs`), not
`EnumWindows`. Filter on visible, not cloaked and a non-empty rect, never on localized
titles. The walk ends early if the window it continues from closes; retry it.

---

## 2026-11-14 `npm test` ran hundreds of extra, failing tests from agent worktrees

**Symptom:** `npm test` reported 65 test files and 112 failures, all under
`.claude/worktrees/<name>/src/...`, while the repo's own 21 files passed.

**Cause:** vitest does not read `.gitignore`. Its default `exclude` covers `node_modules` and
`.git` only, so every worktree checkout under `.claude/worktrees/` is collected as well, each
against its own stale `node_modules`.

**Rule:** keep `.claude/worktrees/**` in `exclude` in `vitest.config.ts`. A tool that walks the
tree (test runner, linter, formatter) needs the worktrees excluded in its own config.

---

## 2026-11-05 The side-by-side password check test failed on macOS CI only

**Symptom:** `windows_are_checked_side_by_side_within_one_budget` fails with
`["the check ran out of its 100 ms budget"]` on the macOS runner, on 2026-11-02 and again
on 2026-11-05. It passes on Windows CI and every local run, and no code in the check changed.

**Cause:** four 40 ms queries side by side in a 100 ms budget leave 60 ms for four thread
starts and the channel. The macOS runner, running the whole test binary in parallel, takes
longer than that to schedule them. Widening the margin to 40 % per query was not enough.

**Rule:** a test that asserts a wall-clock budget gets a budget of its own large enough that
scheduling is noise (here 1 s, queries 400 ms), not the module's 100 ms test budget. Keep
the ratio, so the test still fails if the queries run one after another.

---

## 2026-11-02 The first e2e test failed with "never opened its DevTools port (9333)"

**Symptom:** on CI only the first test of the run fails, 30 s after launch, before any UI
step; the other six launch the same exe and pass. A rerun of the job passes.

**Cause:** the CI runners' first WebView2 start got slow. The same `main` commit took 6 to
12 s for its first test on 2026-10-31 and 35.6 s when rerun on 2026-11-02. The wait was 30 s.

**Rule:** `e2e/app.ts` waits 60 s for the DevTools port, and the test timeout is 90 s. If the
first test fails here again, rerun `main`'s last green e2e job before blaming the branch.

---

## 2026-10-24 The UI shows the main window by itself once, after mount

**Symptom:** an e2e test hid the main window and it was visible again a moment later. Passed
locally, failed on the slower CI runner.

**Cause:** `App.tsx` leaves compact mode on mount (`exitCompactMode`), which calls
`appWindow.show()`. A test that hides the window before the UI has mounted loses the race.

**Rule:** wait for the UI (a visible button) before changing window state in a test, and check the
state still holds a second later. See `e2e/single-instance.e2e.ts`.

---

## 2026-10-24 Every e2e test failed at launch with WebView2 error 0x8007139F

**Symptom:** all e2e tests fail on launch with `0x8007139F`, right after a `cargo test`.

**Cause:** the exe under test had been relinked with the normal identifier, so its WebView2
profile was the installed app's, which was in use. `cargo test` and `tauri dev` both relink the
shared `target/debug` exe.

**Rule:** `npm run e2e:build` builds into `src-tauri/target/e2e`, and the tests run only that exe.
Rebuild it after changing Rust code; never point the tests at `target/debug`.

---

## 2026-10-24 A resampler test measured noise at -60 dB that the resampler did not make

**Symptom:** an alias-level test failed near -60 dB, and no change to the resampler moved it.

**Cause:** the test sine was generated with an f32 phase. The accumulated rounding error is noise
in the *input*.

**Rule:** compute the phase in f64 and convert each sample to f32. The resampler itself measures
below -140 dB (`capture/audio.rs` tests).

---

## 2026-11-01 LNK1318 "Unerwarteter PDB-Fehler: LIMIT (12)" was a full disk

A debug `cargo build` on Windows failed to link with `LNK1318 ... LIMIT (12)` on
`cogniclone_recorder.pdb`. That message reads like a PDB size or record limit, and the timing
pointed the same way (new `windows` crate features had just been added), so the first fix tried
was a larger `/PDBPAGESIZE`. The next build failed earlier with `os error 112` (disk full): D: had
0.22 GB left, because the main checkout's `src-tauri/target` had grown to 70 GB. After `cargo
clean` there, the original build linked unchanged.

**Rule:** on LNK1318 or any odd linker error, check free space on the build drive first
(`Get-PSDrive D`). Each worktree has its own `target/` (about 9 GB here), and the main checkout's
grows without bound; `cargo clean` in an idle checkout is the fix, not a linker flag.

## 2026-11-01 UIA clients created on many threads at once fail with E_FAIL

The password check warms up every visible window at recording start, one thread each. With a
UIA client per thread, all but one `CoCreateInstance(CUIAutomation8)` failed with `0x80004005`
when about 14 ran at once, and the failures were silent: a warm-up does not report, so the only
symptom was every later step flagged "not checked twice". A single query on its own always
worked, which is why the mocked tests and a one-window run passed. Printing the warm-up results
found it.

**Rule:** one UIA client per process (`capture::uia::client`), created once under a lock and shared;
every thread joins the MTA before using it. Client objects created in the MTA are free-threaded.

## 2026-11-01 A process that is not per-monitor DPI aware gets masks beside the field

A live run from `cargo test` saved a login page with its password field unmasked and the step
reported `ok`. UIA had returned the field at (937, 323) while it was on screen at (1406, 484):
the test process was DPI unaware on a 150% display, so UIA reported scaled coordinates while DWM
(window bounds) and xcap (pixels) are always physical. The recorder itself is fine because
`main.rs` sets `PROCESS_PER_MONITOR_DPI_AWARE`, but nothing checked that.

**Rule:** the check refuses to run in a thread that is not per-monitor aware
(`uia::client` returns an error), so every step is flagged instead of masked in the wrong place.
Any live test of the capture path must set DPI awareness first, as `live_password_check` does.
