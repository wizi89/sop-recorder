# Data sources

Where a session looks before it guesses: the app's log, its error reports, and what CI keeps from each run. The factory cannot run the app itself (`AGENTS.md`). On a bug report, ask for the first two from the owner, and read the third yourself.

## The app log

`tauri-plugin-log`, configured in `src-tauri/src/lib.rs`. Files rotate at 5 MB and are all kept.

| Build | Where |
|---|---|
| Release, Windows | `%LOCALAPPDATA%\com.cogniclone.recorder\logs\` |
| Release, macOS | `~/Library/Logs/com.cogniclone.recorder/` |
| Debug | stdout; also `recorder.log` in `COGNICLONE_DEV_LOG_DIR` when that is set and its parent exists |
| e2e build | the release locations, under its own identifier `com.cogniclone.recorder.e2e` (`e2e/tauri.e2e.conf.json`) |

The log holds the input-suppression audit trail (design D4), so a question about dropped clicks is answered here.

## Error reports

`src-tauri/src/error_reports.rs` and `src-tauri/src/error_reports/`. A panic, a failed command or a UI error becomes a JSON report in a queue on disk:

- **Windows:** `%LOCALAPPDATA%\com.cogniclone.recorder\error-reports\`
- **macOS:** `~/Library/Application Support/com.cogniclone.recorder/error-reports/`

Each report carries the last 64 KB of the log (`log_tail`). By design it has no screenshot, audio, transcript, guide, email or token.

A report stays in the queue until the user agrees to send it. It then goes to `POST /client-reports` (`docs/server-contract.md`). Sent reports live on the server, which this repo cannot read: ask the owner for one.

## CI artifacts

From `.github/workflows/ci.yml`, per run:

| Artifact | When | What |
|---|---|---|
| `e2e-screens` | every e2e run, kept 30 days | the app window at the end of each journey in `e2e/login.e2e.ts`, one PNG per test, named after its title (`e2e/app.ts`, `captureScreen`) |
| `e2e-results` | e2e failed, kept 7 days | Playwright's failure output |
| installers | builds with installers (main, manual runs, `[build]` commits) | the Windows and macOS bundles |

Read them with `gh run download <run-id> -n <artifact>`, or through the REST API (`gh api repos/wizi89/sop-recorder/actions/runs/<run-id>/artifacts`) in a cloud session.

### Before and after screens for a UI PR

On a PR, the `e2e` job's summary carries two links:

- **Before:** main's latest `e2e-screens`.
- **After:** this run's `e2e-screens`.

A PR that changes what a screen shows links that summary under **Evidence**, and names the PNGs to compare. If no journey reaches the changed screen, add a step to one that does, or a new journey, so its end state shows the change. Otherwise list the visual check under **Not verified here**.
