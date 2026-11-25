# Findings

Hard-won, non-obvious facts about the recorder. Each entry cost real debugging time.
Read this before diagnosing anything in capture, window management, or permissions.

**Adding entries:** newest at the top. One entry per finding. Keep the shape
`Symptom / Cause / Rule`, because the symptom is what a future reader will search for.
Only add things that were genuinely surprising: if it is obvious from the code, skip it.

Older entries (2026-09 and earlier): [findings/2026-09-and-earlier.md](./findings/2026-09-and-earlier.md).

---

## 2026-11-25 Out-of-context foreground events can arrive after a later switch

**Symptom:** Alt+Tab to an app run as administrator: the watch logs it in front, then ends the
visit about 1.5 ms later while the window stays in front for seconds, so no bar hint and no limbo
row ([#108](https://github.com/wizi89/sop-recorder/issues/108)).

**Cause:** `EVENT_SYSTEM_FOREGROUND` hooked with `WINEVENT_OUTOFCONTEXT` is delivered
asynchronously. The event for the Alt+Tab switcher can land after the one for the window that
replaced it, and the watch classified the event's `hwnd`.

**Rule:** a foreground event decides from `GetForegroundWindow()`, the event's `hwnd` only when
nothing is in front (`deciding_window` in `capture/unrecorded.rs`).

---

## 2026-11-23 Review rules wanted more than once are still missing from CODING_STANDARDS.md

Moved: the wanted rules are in "Not yet a rule" in `CODING_STANDARDS.md`.

Pattern candidate: Review's verdict lists the rules it wanted, and nothing copies them into the
repo's "Not yet a rule", so the owner never sees them to promote.

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

Pattern candidate: issues written from a PR review cite the PR head's lines, not `main`'s.

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

Graduated: the `actionlint` job in `.github/workflows/ci.yml`.

---

## 2026-11-23 Saving boxes over a read-only `redactions.json` fails on Windows, works on macOS

**Symptom:** with `redactions.json` set read-only, saving a step's boxes in the review screen
fails with "redactions.json konnte nicht geschrieben werden" on Windows. The same test on macOS
saves without an error and the file holds the new boxes.

**Cause:** the write is a temp file renamed over the target (`tempfile.persist` in
`src-tauri/src/output/redactions.rs`). On Windows the rename (`MoveFileEx` with replace) refuses
to replace a read-only file. On macOS (POSIX `rename`) what counts is write permission on the
folder, not on the file, so the read-only flag on the old file does not stop it.

**Rule:** a "read-only file blocks the write" test or manual check holds only on Windows. On
macOS and Linux, make the write fail another way (a read-only folder, or a directory in the
file's place).

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

Pattern candidate: `security-events: read` does not cover Dependabot alerts; they need `vulnerability-alerts: read`.

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

Pattern candidate: tools that walk the tree collect `.claude/worktrees/` checkouts unless their config excludes them.

---

## 2026-11-02 The first e2e test failed with "never opened its DevTools port (9333)"

**Symptom:** on CI only the first test of the run fails, 30 s after launch, before any UI
step; the other six launch the same exe and pass. A rerun of the job passes.

**Cause:** the CI runners' first WebView2 start got slow. The same `main` commit took 6 to
12 s for its first test on 2026-10-31 and 35.6 s when rerun on 2026-11-02. The wait was 30 s.

**Rule:** `e2e/app.ts` waits 60 s for the DevTools port, and the test timeout is 90 s. If the
first test fails here again, rerun `main`'s last green e2e job before blaming the branch.

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

Pattern candidate: LNK1318 "LIMIT (12)" from the MSVC linker means a full disk, not a PDB limit.

## 2026-11-01 A process that is not per-monitor DPI aware gets masks beside the field

Graduated: `src-tauri/src/capture/uia.rs` (the UIA client refuses to run without per-monitor
DPI awareness).
