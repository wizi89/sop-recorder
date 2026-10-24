# Archive note -- browser-login

## The follow-up change's first task (task 8.1)

> **Superseded 2026-10-24:** the fallback stays, with no removal date. The
> signal was added instead -- a failed browser sign-in now creates an error
> report (spec `user-authentication`, "A failed browser sign-in is
> reported"). Removal is a later decision on those reports.

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

## What the windowed runs found that no test could (2026-10-19)

Two defects that every automated check passed over, both found by using the
thing:

- **The browser kept the code and lost the answer.** `read_request_line` parses
  the request line and stops, so the browser's headers stayed unread in the
  receive buffer. Closing a socket with data queued sends an RST rather than a
  FIN; on a reset the browser discards the 302 it has already read and retries
  the navigation, and by then `recv` has dropped the listener -- so the tab
  showed `ERR_CONNECTION_REFUSED` while the app signed in normally. Whether it
  bit depended on how much was still unread at close, which is why it
  intermittently worked. Measured against Chromium before changing anything: a
  server that stops at the request line gets `ERR_CONNECTION_RESET`, the same
  server draining first follows the redirect. The connection is now drained
  after it is answered, and the regression test fails without that
  (`WSAECONNRESET` 10054).
- **The wait had no screen.** `loginViaBrowser` sets `loading` as well as
  `awaitingBrowser`, and App's loading gate returns before `LoginScreen` is
  reached, so the five-minute wait showed only "...". The hint and the cancel
  button existed and were rendered nowhere -- the only way out was the timeout.
  The `LoginScreen` tests pass `awaitingBrowser` straight in and so could not
  see it; the test that now covers it drives App with `login_via_browser` left
  pending.

Both needed a real browser and a real window. The first also needed the
browser's own retry, which no test client performs.

## The failure paths, as they actually behave (task 7.4)

Run on Windows against deployed staging, 2026-10-19:

- **Cancel** ends the attempt at once and the tab gets the failure page.
- **Timeout** was run against a temporarily shortened bound (60s rather than
  300s, reverted straight after): the attempt ended by itself at 61 seconds
  with "Die Anmeldung im Browser wurde nicht abgeschlossen", and `netstat`
  showed the port unbound. What this does not prove is the value 300 itself,
  only the path it leads to.
- **Closing the tab does nothing, and nothing is what it can do.** No
  connection is ever made to the loopback port, so the recorder has no signal
  to react to; it waits out the bound. The user's way out is the cancel
  button, which is why the screen it lives on mattered more than it looked.
- **A tampered callback** (`state=wrong`, opened by hand while an attempt was
  pending) left the recorder signed out with the verification message, sent
  the tab to `sign-in-failed?reason=verification`, and the API log shows no
  exchange at all -- the forged code was never offered upstream.

**A stale tab can still mint a code.** Confirming in a tab whose attempt has
already failed issues a fresh code and redirects to a port nobody is listening
on: the server cannot know the recorder gave up. The code is single-use, lives
60 seconds, and is bound to a PKCE verifier that only the dead attempt ever
held, so it is unusable by anyone including the user. Recorded so it is not
rediscovered as a suspected leak.

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
| **Safari 26.6.2** (WebKit), macOS 26.6.2, 2026-10-19 | Followed **silently** -- no prompt, no block. Default browser confirmed before the run; no error report followed. |
| **Firefox 158.0** (Gecko), macOS 26.6.2, 2026-10-19 | Followed **silently** -- no prompt, no block. First start, no saved session; the sign-in completed and no error report followed. |

**Task 8.2 is not triggered by anything observed, on all three engines.** The loopback listener
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

macOS 26.6.2, `tauri dev --staging`, 2026-10-18: sign-in, logout and
a second sign-in all completed, the Keychain held the credentials, and a
restart went straight to the recorder. `cargo clippy -D warnings` had only
ever run on Windows and failed on the `cfg(target_os = "macos")` arms; fixed.
Which browser that run used was not recorded with certainty -- the system
default turned out to be Brave -- so Safari was run again on its own on
2026-10-19.

**Every sign-in filed an error report.** Not a crash: the screen switch after
sign-in tears down Tauri event listeners while some are still registering,
Tauri's unlisten rejects asynchronously, and the global `unhandledrejection`
handler files that as a `ui_error`. `safeUnlisten` already existed for this;
three hooks called the late-path unlisten raw instead, and two
`onFocusChanged` subscriptions could not be protected at all, because the
combined unlisten it returns discards the promises of the two it wraps. Those
now listen to `TauriEvent.WINDOW_FOCUS` directly. Pre-existing on `main`, but
the browser sign-in is what made it fire on every sign-in.

## Before merging: the old-server case was detected too late

Checked against production on 2026-10-19: neither `/auth/desktop/exchange` on
the API nor `/desktop/authorize` on the web app exists there yet (both 404).
The unsupported-server message (task 7.7) only fired at the exchange, and the
exchange never happens against an old server: the browser lands on the web
app's 404 page, no callback arrives, and the attempt ends in a timeout five
minutes later. Two changes close that:

- **`probe_exchange`** posts an empty body to the exchange endpoint before
  anything is bound or opened. A server with it refuses that with a JSON
  `invalid_code`; one without it answers 404. The user gets the
  unsupported-server message and the password form at once. The probe shares
  `lacks_exchange` with `complete`, and anything inconclusive -- no network, a
  timeout -- lets the attempt go ahead as before.
- **`release.yml` has a `server-contract` job** that every release, beta
  included, has to pass: it reads the production URLs from `config.rs` and
  fails if either endpoint is missing. "The server must ship first" is now
  enforced at the tag rather than remembered. New server dependencies get one
  line there.

