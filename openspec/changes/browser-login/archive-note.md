# Archive note -- browser-login

## The follow-up change's first task (task 8.1)

Remove the password fallback, the `login` command, `net_auth::sign_in` and
their copy, one release after this ships.

**The trigger, recorded honestly: support silence, not a measured signal.**
Nothing in this change reports a failed browser sign-in anywhere but the local
log. The blind spot is the EDR and Local-Network-Access risks in design.md,
which are checked once by hand at release time (task 7.1b) and never again. If
that is not acceptable when the time comes, the follow-up's first task is to
**add the signal**, not to remove the fallback.

## What the deep-link switch costs (task 8.2)

- Delete `src-tauri/src/network/loopback.rs` -- the whole file, `Reply`
  included. The deep-link receiver yields a `Reply` whose `finish` does
  nothing, because under a scheme the browser has no connection waiting on the
  app, and the web page shows its own neutral "return to the app" state.
- Add `tauri-plugin-deep-link` and `tauri-plugin-single-instance`.
- Add `plugins.deep-link.desktop.schemes` to `tauri.conf.json`.
- Call `register_all()` under `#[cfg(all(debug_assertions, windows))]` for dev.
- Point `start_deep_link()` at the plugin's `on_open_url`, returning the same
  `Callback` shape.

Nothing in `network/auth.rs`, the server or the UI changes. `parse_callback`
already treats both URL shapes identically, and there is a test asserting
exactly that. The web app's allowlist already accepts
`cogniclone://auth/callback`.

## What the implementation found that the design did not

- **A sequential accept loop is not enough.** The design argued correctly that
  a one-shot `accept()` is wrong, and the first implementation of the loop was
  still wrong: it read each connection to completion before accepting the
  next, so a silent connection held the attempt for the full 5-second read
  deadline. Task 4.5's test caught it. Connections are now read concurrently
  and the first well-formed callback wins.
- **A request line that arrives but is unusable is not the same as silence.**
  The first implementation dropped both silently, so `GARBAGE\r\n\r\n` got no
  400 and task 4.3 failed. The reader now distinguishes "nothing before the
  deadline" (drop; there is nobody to answer) from "a request line we do not
  serve" (400; there is a real client waiting).
- **`CancellationToken` is not comparable**, so "clear the slot only if it
  still holds *this* attempt" needs an attempt id. Without it a superseded
  attempt finishing last would clear the live attempt's token, and a later
  cancel would do nothing.

## Verified against a real stack

The server side was run locally (`uvicorn` + `next dev`) against the **real
staging Supabase project** with the real smoke account:

- The recorder's own `complete()` exchanged a real code for a real session,
  and the replay of that code was refused (tasks 7.5, 7.6). See
  `src-tauri/tests/live_exchange.rs`, which is `#[ignore]`d and carries the
  recipe.
- The desktop session survives a web sign-out, and both sessions refresh
  independently (task 7.6, and `desktop-browser-login` 5.2b).

## Not verified here, and why

These need a person at a desktop, and in one case a Mac:

- **7.1 / 7.2** -- a full windowed sign-in on Windows and macOS against
  staging, including the keyring and a restart with no login screen.
- **7.1b** -- the browser matrix (Chrome, Edge, Firefox, Safari): whether the
  `302` from an `https` page to `127.0.0.1` goes through silently, prompts, or
  is blocked. **Any prompt or block is the trigger for task 8.2**, and the
  result belongs in this note with the browser and version.
- **5.1b** -- that the window actually comes to the front (and that a flashing
  taskbar button on Windows is what happens when foreground-lock rules apply).
- **7.3 / 7.4 / 7.4b / 7.4c** -- the already-signed-in path, the failure paths
  by hand, the wrong-account case, and the decline, each end to end through a
  real browser. Their mechanisms are unit-tested (the decline survives as
  `CallbackError::Denied` with its own message and `reason=denied`; a tampered
  `state` fails verification without exchanging), but the user-visible pass is
  still owed.
- Staging was never pointed at either branch: Coolify rejected the stored API
  token with 401.
