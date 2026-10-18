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

- **7.1b** -- Firefox and Safari. Brave is done (see below); whether the `302`
  from an `https` page to `127.0.0.1` goes through silently, prompts, or is
  blocked still has to be checked on a Gecko and a WebKit browser. **Any
  prompt or block is the trigger for task 8.2**, and the result belongs in
  this note with the browser and version.
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

## The browser matrix, so far (task 7.1b)

| Browser | Result |
|---|---|
| **Brave** (Chromium, shields on by default), Windows, 2026-10-17 | The `302` from the `https` page to `127.0.0.1` was followed **silently** -- no prompt, no block, no Local-Network-Access interstitial. |
| Chrome, Edge | Not tested. Brave is the strictest Chromium of the three by default, so a silent pass there is weak but real evidence for the others. |
| **Safari** (WebKit), macOS 26.6.2, 2026-10-18 | Followed **silently** -- no prompt, no block. |
| Firefox (Gecko) | Not tested, and not predicted by the other results -- a different engine with its own local-network policy. |

**Task 8.2 is not triggered by anything observed.** The loopback listener
stays; the deep-link switch stays on the shelf with its cost written down
above.

## The full windowed pass (task 7.1)

Windows, against deployed staging, 2026-10-17, in Brave: the browser opened,
**a password manager filled the form** -- the entire reason this change
exists -- the recorder landed on the recorder screen, the keyring held
`refresh-token` and `email`, and a restart went straight to the recorder with
no login screen.

## The wrong-account case, as it actually behaves (task 7.4b)

Exercised on 2026-10-17: signed out of the recorder, started a browser
sign-in, and the browser held a different account. The consent step named
that account and offered "Nicht du? Konto wechseln", which is what the design
specifies.

What the run established, and what it rules out: **the page cannot warn about
a mismatch, and neither can the recorder.** Signing out clears both keyring
entries, so at the moment the consent screen is shown the recorder holds no
account at all -- there is nothing on either side to compare against. Passing
the recorder's account as a hint would do nothing for the ordinary path,
where the user signs out first. The consent screen naming the account is
therefore not one defence of several; it is the only one, and the recorder's
main screen showing the signed-in email is the sole check after the fact.

Wording that puts the account inside the decision (a confirm button reading
"Als <account> anmelden") was written and then reverted: the maintainer looked
at it against the live page and chose to keep the current copy. Recorded here
so the question is not reopened as if it had never been asked.

## The macOS pass (task 7.2)

macOS 26.6.2, `tauri dev --staging`, Safari, 2026-10-18: sign-in, logout and
a second sign-in all completed, the Keychain held the credentials, and a
restart went straight to the recorder. `cargo clippy -D warnings` had only
ever run on Windows and failed on the `cfg(target_os = "macos")` arms; fixed.

**Every sign-in filed an error report.** Not a crash: the screen switch after
sign-in tears down Tauri event listeners while some are still registering,
Tauri's unlisten rejects asynchronously, and the global `unhandledrejection`
handler files that as a `ui_error`. `safeUnlisten` already existed for this;
three hooks called the late-path unlisten raw instead, and two
`onFocusChanged` subscriptions could not be protected at all, because the
combined unlisten it returns discards the promises of the two it wraps. Those
now listen to `TauriEvent.WINDOW_FOCUS` directly. Pre-existing on `main`, but
the browser sign-in is what made it fire on every sign-in.
