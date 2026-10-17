## Context

See `proposal.md` -- Why. This section records only the implementation facts the approach depends on. Line references are against v0.19.0-rc.1 (`d1a5b54`).

- **Recorder login today.** `LoginScreen.tsx` collects email and password and calls `useAuth.login`, which invokes the `login` command (`commands/auth.rs:41`). That calls `net_auth::sign_in` (`network/auth.rs:78`), which POSTs `{email, password}` to `{api_base}/auth/signin`, saves `refresh-token` and `email` to the OS keyring, and returns an `AuthSession { access_token, email }`. The access token lives in `SessionCache` in memory; the refresh token in the keyring under `cogniclone` (release) or `cogniclone-dev` (debug).
- **Session restore.** `refresh_session` (`network/auth.rs:118`) POSTs the stored refresh token to `/auth/refresh` and re-saves whatever comes back; when the server rejects the token it clears the keyring entry and reports no session, and `useAuth` -- which calls it once on mount -- shows the login screen. It emits nothing. The `auth:session_expired` event that `useAuth` also listens for is emitted elsewhere, on a 401 during generation (`commands/generate.rs:303`, `:327`) and from `SettingsPage.tsx:138`; both paths end at the same login screen.
- **The API is a Supabase proxy.** `/auth/signin` and `/auth/refresh` (`9_sop-sorcery/server/main.py:310` and `:346`) forward to Supabase's `/auth/v1/token` and return `{access_token, refresh_token, email}`. `/auth/signin` is rate-limited per IP by an in-process deque (`main.py:254`) -- a precedent for short-lived in-process auth state on this server.
- **The web app already authenticates.** `9_sop-sorcery/web/src/app/login/page.tsx` signs in with `supabase.auth.signInWithPassword` in the browser and redirects to `/dashboard`. A user with a live cookie session is already authenticated when the recorder sends them there.
- **The recorder already opens browser URLs.** `LoginScreen.tsx:69` and `:85` use `tauri-plugin-opener` with `getWebappUrl()` for "forgot password" and "create account". Nothing new is needed to launch the browser.
- **Constraints.** The app ships Windows (NSIS) and macOS (universal). `url = "2"` and `uuid` (v4) are already dependencies. The Tauri capability set (`src-tauri/capabilities/default.json`) already grants `opener:default`.

## Goals / Non-Goals

**Goals:**

- Credentials are entered only in a real browser, where the user's password manager works.
- The recorder never holds a password, and holds the authorization code for as long as one HTTP round-trip.
- The security design is final at v1: swapping loopback for a custom scheme later must not require revisiting PKCE, the code lifetime, the redirect allowlist or the server contract.
- Every piece of logic that is not the transport is testable without a browser, a socket or a network: pure functions over `Url` and strings.

**Non-Goals:**

- A trait with two implementations for the callback source. There is one implementation; the seam is a function boundary, and the second implementation is when it earns an abstraction.
- Persisting the pending login across a process restart. With the `single-instance` plugin (the deep-link path), the second process forwards its argv to the running one, so the waiting task is always in the original process. Nothing needs to survive a restart in either mechanism.
- Handling more than one sign-in attempt in flight. The button is disabled while waiting, and there is one `PendingLogin` at a time, so a second attempt **supersedes** the first: starting one cancels the attempt in flight through *that attempt's* token (dropping its listener) before installing a fresh token and minting new `state` and verifier. Without that, the first attempt's callback would be checked against the second attempt's `state` and the user would be shown the tampering message for a sign-in nobody tampered with.

## Decisions

### Loopback now, custom scheme later, one function between them

`start_loopback()` returns a handle carrying the redirect URI it bound and a receiver that yields the callback `Url`. It is the only code that knows the mechanism:

```rust
// network/loopback.rs -- the file that is deleted when the scheme lands
pub struct Callback { pub redirect_uri: String, rx: oneshot::Receiver<(Url, Reply)>, .. }
pub struct Reply(..); // holds the browser's connection open until finish()
pub async fn start_loopback() -> Result<Callback, String>;
impl Callback { pub async fn recv(self, timeout: Duration, cancel: &CancelToken) -> Result<(Url, Reply), String>; }
impl Reply { pub async fn finish(self, outcome: &Outcome); } // 302 to the web app's result page
```

```rust
// commands/auth.rs -- unchanged by the switch
let (pending, challenge) = net_auth::begin();
let cb = loopback::start_loopback().await?;
open_url(format!("{webapp}/desktop/authorize?redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
                 enc(&cb.redirect_uri), pending.state, challenge))?;
let (url, reply) = cb.recv(Duration::from_secs(300), &cancel).await?;
let result = select! {                      // the exchange is bounded and cancellable too
    r = async {
        let (code, state) = net_auth::parse_callback(&url)?;
        if !constant_time_eq(&state, &pending.state) { return Err(..); }
        net_auth::complete(&pending, &code, api_base).await   // returns tokens, persists nothing
    } => r,
    _ = sleep(Duration::from_secs(15)) => Err(timeout_err()),   // 10 server < 15 here < 20 finish
    _ = cancel.cancelled() => Err(cancelled_err()),             // this attempt's token, not a shared Notify
};
// one place decides the outcome, outside the cancellable future, before the tab is answered
let outcome = match result {
    Ok(_) if cancel.is_cancelled() => Err(cancelled_err()),     // cancel arrived as the exchange resolved
    Ok(t) => save_to_keyring(t),                                // a keyring failure is a failed sign-in
    Err(e) => Err(e),
};
reply.finish(&Outcome::from(&outcome)).await;                   // bounded at 20s from the callback
let session = outcome?;
bring_to_front(&window);
Ok(session)
```

The deep-link version replaces `start_loopback()` with one that returns `cogniclone://auth/callback` and a receiver fed by the plugin's `on_open_url`, and a `Reply` whose `finish` does nothing -- under a scheme the browser has no connection waiting on the app, and the web page shows its own neutral "return to the app" state. `parse_callback` is unaffected: `url::Url` parses `cogniclone://auth/callback?code=..&state=..` and `http://127.0.0.1:53124/callback?code=..&state=..` into the same query pairs. The callback path is `/callback` in both, deliberately.

*Alternative considered:* ship the custom scheme now, as Postman does. Rejected for this release -- Postman already needed `postman://` for "Run in Postman" buttons, so login rode on a handler that existed. Here it would mean an OS registration that differs per platform (HKCU registry on Windows, `Info.plist` on macOS), no scheme at all under `tauri dev` on macOS, plus the `single-instance` plugin to route a second process's argv into the running one -- a startup-path change to an app that holds live recorder state. Loopback needs no registration, behaves identically in dev and release, and is what RFC 8252 section 7.3 recommends precisely because any local application can claim a private URI scheme.

*Alternative considered:* skip the code and let the browser redirect with the refresh token in the query. Rejected -- the token would land in browser history, in the web app's server logs, and in any referrer, in exchange for saving one HTTP request.

### PKCE at v1, even though loopback barely needs it

`begin()` mints `state` and `verifier` from two `uuid::Uuid::new_v4()` values each (122 bits of randomness apiece; the hex verifier is 64 characters, inside PKCE's 43-128). The challenge is `base64url(sha256(verifier))`, unpadded.

Under loopback, PKCE guards little: the app binds the port before the browser is opened, so an attacker cannot be listening on it. Under a custom scheme it is the primary defence -- on Windows the scheme is a `HKCU` registry key that any user-level process can overwrite, so a hostile app can receive the code. Adding it then means a coordinated change to the recorder, the web app and the API *simultaneously*, because `code_challenge` has to start being sent before it can start being required. Adding it now costs `sha2`, `base64` and about fifteen lines, and makes the later switch a recorder-only change.

*Alternative considered:* `rand` for the verifier. Rejected -- `uuid` v4 is already a dependency and already CSPRNG-backed.

### The redirect allowlist accepts both shapes from day one

The web app validates `redirect_uri` against exactly two forms: `http://127.0.0.1:<port>/callback` (any port, IPv4 literal only -- never `localhost`, which can resolve through a hosts file) and `cogniclone://auth/callback`. Writing both now means the web app is never touched again for the switch. The second form is unreachable until the scheme exists, and rejecting it later is a one-line revert if the plan changes.

### What carries over

| Piece | Where | Survives the switch |
|---|---|---|
| `/desktop/authorize` + `redirect_uri` allowlist | web app | yes, unmodified |
| `/auth/desktop/code`, `/auth/desktop/exchange` | API | yes, unmodified |
| PKCE `begin()` / verifier / challenge | recorder | yes |
| `state` generation and comparison | recorder | yes |
| `parse_callback(&Url)` | recorder | yes |
| `complete()` -> keyring -> `SessionState` | recorder | yes |
| Login UI, waiting state, cancel, timeout, copy | recorder | yes |
| `/desktop/signed-in`, `/desktop/sign-in-failed` result pages | web app | yes |
| Bringing the window to the front on success | recorder | yes |
| `start_loopback()`, `Reply` | recorder | **no -- ~60 lines, deleted** |

### The listener is hand-rolled, not a web framework

The listener **accepts connections in a loop** until a well-formed `/callback` arrives or the attempt ends. For each one it reads the request line under a 5-second read deadline, parses the target with `url::Url`, and resolves the attempt with the URL and that connection still open (see "The browser tab reports the outcome the recorder reached"). A connection that is malformed, that targets another path, or that sends nothing before the deadline is answered 400 where possible, dropped, and the loop continues waiting. Roughly 50 lines over `tokio::net::TcpListener`, which is already in the tree via `tokio` with `features = ["full"]`.

A single `accept()` would be simpler and is wrong: browsers routinely open speculative and preconnect TCP connections that carry no request bytes, and any local process can connect to the port. Under one-shot accept, such a connection consumes the attempt, and -- with no read deadline -- blocks on a read while the real callback lands on a connection that is never accepted. The user then waits out the full five minutes on the one screen the app has nothing else to offer, intermittently and unreproducibly. The loop and the per-connection deadline are what make the "not a general-purpose server" simplification safe rather than merely small.

*Alternative considered:* `axum` or `tiny_http`. Rejected -- a dependency whose entire job is to parse one request line, in code scheduled for deletion. Anything the listener cannot parse is answered with 400 and ignored; it is not a general-purpose server and does not need to be.

Binding `127.0.0.1:0` lets the OS choose the port, which avoids both a fixed-port collision and the need to probe. Binding `127.0.0.1` rather than `0.0.0.0` means the socket is not reachable from the network.

### The browser tab reports the outcome the recorder reached

The earlier draft answered the callback with a fixed 200 "you can close this tab" page *before* the `state` check and the exchange. Every failure after that point -- a mismatched `state`, an expired or replayed code, an API without the exchange endpoint -- then showed success in the browser and an error in the app, and the user believed the tab. The tab is where the user is looking at that moment, so it has to tell the truth.

The listener therefore holds the callback connection open, the command validates and exchanges, and `Reply::finish` answers with a `302` to a page the web app hosts:

- success -> `{webapp}/desktop/signed-in`
- failure -> `{webapp}/desktop/sign-in-failed?reason=<verification|denied|expired|unsupported|error>`

`reason` is a fixed token, never a server message, so nothing user- or token-derived lands in browser history. The pages, not the recorder, carry the branding, the signed-in account (read from the browser's own web session, so no email travels in the URL), a "Not you? Switch account" link, help, and a "go back to the app and try again" hint. Copy and design change in the web app without a recorder release.

Two things in that sketch are deliberate and were each wrong in an earlier draft. The cancel token is re-checked **after** the `select!` has resolved: `select!` picks a ready branch, and when the exchange's response and the cancel become ready in the same wakeup tokio chooses between them at random, so winning the branch is not evidence that the attempt was not cancelled. Without the re-check a cancel landing in that window leaves a refresh token in the keyring behind a UI that said cancelled, and `refresh_session` restores it silently on the next launch -- the failure this section claims to eliminate, surviving at a narrower width. And the keyring write happens **before** `finish`, not after, so that a keyring failure (locked keychain, credential-manager error) reaches the browser as `reason=error` instead of sending the tab to the signed-in page for a sign-in the app is about to report as failed. An earlier draft answered the tab first on the grounds that the tab is bounded and the user is looking at it; that is true, and the write is a local credential-store call of a few milliseconds, so it costs the `finish` budget nothing measurable -- while the honesty of the result page is a stated requirement.

The exchange is one HTTP round-trip, normally well under a second. `finish` is bounded at **20 seconds**, and the number is agreed with the other side rather than guessed. The rule is an ordering, not a pair of numbers, and it has three terms, because `finish` is called only *after* the exchange has returned:

> server upstream bound **<** recorder exchange deadline **<** recorder `finish` bound

Today that is 10 < 15 < 20. An earlier draft had 15 against a server worst case of 30 (two blocking Supabase calls at `timeout=15` each), which is backwards; the draft after it set the exchange deadline and the `finish` bound both to 20, which is also wrong, just more narrowly -- an exchange succeeding in the last milliseconds before 20s loses to a `finish` timer that started at the same instant, and the tab reports failure while the app writes the keyring and raises its window. That is the exact inversion this section exists to forbid, so the ordering is strict at every step. If the exchange has not resolved by then, the browser is sent to the failure page with `reason=error` and the app reports its own result when it arrives. A `Reply` dropped without `finish` -- cancel, a panic -- answers the same failure redirect, so the browser never hangs on the connection.

*Alternative considered:* keep a static page in the recorder and render success or failure there. Rejected -- it duplicates the web app's branding in a file scheduled for deletion, and cannot show the account without passing the email through the URL.

### The recorder comes to the front when sign-in completes

On success the command calls `unminimize()`, `show()` and `set_focus()` on the main window, because at that moment the user is in the browser and the recorder is behind it. The web app's success page says the recorder is now open; this makes that true. On Windows, foreground-lock rules may turn `set_focus()` into a flashing taskbar button instead of a raised window; that is accepted, and still points the user at the app.

### Cancel and timeout are first-class, in both phases

The command selects over three futures: the callback, a five-minute timeout, and a cancel signal that `cancel_browser_login` fires. All three drop the listener.

**The cancel handle is created with the attempt and belongs to it.** This matters more than it looks, because the handle is now load-bearing three times over: twice within an attempt (waiting for the callback, then waiting for the exchange) and once across attempts (a second sign-in supersedes the first). A single long-lived `tokio::sync::Notify` in app state does **not** work for that: `notify_one()` with no waiter stores a permit, and the next `notified().await` returns immediately. Superseding as described below fires the cancel unconditionally at the top of every attempt, so on the ordinary path -- previous attempt cancelled or timed out, user clicks sign in again -- the permit would be stored with nobody waiting, and the new attempt would cancel itself on its first poll. Every retry after that does the same, and browser sign-in is dead until the app restarts.

So app state holds a `CancellationToken` (or an equivalent one-shot) **created when an attempt starts and replaced, not re-signalled, when the next one starts**. Cancelling takes whatever token is in state at that moment and cancels it; a cancel aimed at an attempt that has already ended is a no-op, because that attempt's token is gone. Superseding cancels the *previous* token before installing the new one, which is a different act from firing a shared signal and does not leave a permit behind.

**The exchange is inside the same `select!`, not after it.** An earlier draft awaited `complete()` once the callback had arrived, with nothing watching it. The exchange is the one remaining network call, `sign_in`'s precedent for building it is `reqwest::Client::new()` with **no timeout at all** (`network/auth.rs:78`; the clients that do set one are in `jobs.rs`, `reports.rs` and `upload.rs`), and a backend that accepts the connection and never answers would leave the user on the waiting screen indefinitely. Worse, Cancel would then signal nobody: the UI returns to the login screen while the command later succeeds, writes the keyring and raises the window -- the app signed in and the user told it was not. So `complete()`'s client carries a 15-second timeout, and the attempt's cancel token and a 15-second deadline stay in the `select!` across it. A cancel after the callback resolves the attempt as cancelled, and the exchange's result is discarded without touching the keyring.

**That last clause only holds if persistence happens outside the cancellable future**, which is the opposite of how `sign_in` is built: it writes the keyring itself, before returning. If `complete()` copied that, the HTTP response landing in the same wakeup as the cancel would mean the keyring was already written when the cancel branch won -- the UI reporting cancelled, nothing clearing the entry, and `refresh_session` quietly restoring the session on the next launch. So `complete()` **returns the tokens and persists nothing**; the command writes the keyring only after the `select!` has resolved in the exchange's favour. Without the cancel the user has no way out of a login they abandoned in the browser except waiting out the timeout or killing the app -- and the login screen is the one screen where the app has nothing else to offer.

### The password form stays for one release

Behind a small "sign in without a browser" link, calling the existing `login` command unchanged. The browser handoff has a failure mode the password form does not -- a browser that cannot reach `127.0.0.1`, an endpoint-security product that blocks a desktop app from listening, a default browser that is not configured -- and the blast radius is total: the user cannot use the product at all. One release of insurance is cheap; the removal is already written down as the first task of the follow-up change.

## Server contract

**`9_sop-sorcery`, change `desktop-browser-login`, is authoritative for everything server-side** -- the authorize route, how a code is minted and stored, and what session the exchange returns. This section states only the surface the recorder itself touches, and must not be read as a specification of the other two. Where the two documents disagree, that one wins; a contradiction between them is a defect in this one.

**`GET {webapp}/desktop/authorize?redirect_uri&state&code_challenge&code_challenge_method=S256`** -- what the recorder opens. It validates `redirect_uri`, treats `state` as opaque and echoes it byte-for-byte, obtains a code, and `302`s to `{redirect_uri}?code={code}&state={state}`. **When the user declines, or the code cannot be minted, it `302`s to `{redirect_uri}?error=<access_denied|server_error>&state={state}` instead** -- the recorder is waiting on a screen with nothing else to offer, and the only alternative is for it to spin out the full five minutes with no explanation. This is safe because the web app validates `redirect_uri` against its allowlist on **every** request that issues a redirect, the confirmation submission included (that change owns the check), and it is why `parse_callback` handles an `error` parameter (task 2.1) and why `denied` is one of the outcome tokens. A decline with no redirect at all would make both dead code. Whether the user is asked for credentials, and whether they are asked to confirm the sign-in, are decided by that change, not this one; the recorder's behaviour is identical either way.

**`POST {api}/auth/desktop/exchange`** -- what the recorder calls. Body `{code, code_verifier}`; returns the same `{access_token, refresh_token, email}` shape `SignInResponse` already deserializes (`network/auth.rs:64`), so the recorder's response handling is unchanged. The recorder relies on three properties of it:

- The code is single-use and short-lived, and is refused without a verifier matching the challenge the recorder sent.
- **The returned session is independent of the browser's**: refreshing or ending one never invalidates the other. The recorder treats a failed refresh as permanent expiry (`network/auth.rs`, `refresh_session`: it clears the stored refresh token and reports no session, which puts the user back on the login screen), so a shared, rotating refresh token would log recorder users out every time they used the web app. This is the property the recorder cannot verify from its own code, and it is why task 7.6 exercises it explicitly.
- A recorder never calls `/auth/desktop/code`, and holds no credential that would let it mint a code for itself.

**`GET {webapp}/desktop/signed-in`** and **`GET {webapp}/desktop/sign-in-failed?reason=..`** -- where the recorder sends the browser once it knows the outcome. Static from the recorder's point of view: it relies only on their existence and on `reason` accepting the five tokens above. They show the account of the browser's web session and a "Not you? Switch account" link. Owned by `desktop-browser-login`.

**The consent step shows the account.** The confirmation `/desktop/authorize` shows before redirecting names the account being signed in, shows the loopback port the code would go to, and offers "Not you? Switch account". That is where a wrong-account sign-in is stopped; the recorder cannot know which account the user intended. The prompt deliberately does **not** claim to have identified the application asking -- nothing in the flow authenticates the caller, and a prompt that implied otherwise would lend its authority to an identical one raised by a hostile local process.

*The earlier draft of this section specified `/auth/desktop/code` as taking the browser's refresh token. That was written before the session-independence decision and is wrong; it described exactly the shared-token design `desktop-browser-login` rejects. It is removed rather than corrected, because this document should not be restating a contract it does not own.*

## Risks / Trade-offs

- **Release order.** A recorder shipped before the API has `/auth/desktop/exchange` cannot sign in by the new path. Mitigated by the fallback link, by tasks 7.1 and 7.2, which verify the new client against the deployed staging API before the recorder is tagged, and by task 7.7, which exercises the recorder against an API that does *not* have the endpoint.
- **Endpoint-security products.** A desktop app binding a loopback port is unusual enough that some EDR configurations flag it. This is the strongest argument for the custom scheme, and the reason the switch is designed for rather than dismissed. If it shows up in support, the scheme becomes the next change rather than an emergency.
- **The browser is a second window the app does not control.** A user who closes the tab without signing in gets a timeout rather than an error; accepted, the timeout message names the cause and the retry. A user signed in to the web app as a *different* account would, with the mandatory consent step, sign the recorder in as that account -- not time out. Mitigated twice: the consent step names the account with a "Not you? Switch account" link (server contract), and the recorder already shows the signed-in email on its main screen (`RecorderScreen.tsx:169`), with sign-out one click away.
- **Browsers restricting public sites from reaching loopback.** Chromium's Local Network Access work gates requests from public origins to `127.0.0.1` behind a permission. Whether a top-level `302` navigation from our `https` web app to the loopback callback is covered, prompts, or is blocked, varies by browser and version and is not something this design can settle on paper. Tasks 7.1 and 7.2 check it against the current Chrome, Edge, Firefox and Safari. If any of them blocks or prompts, that is the trigger for the custom scheme (task 8.2), and the fallback link covers users until it ships.
- **The code travels through a `302` in the user's browser.** It is single-use, 60 seconds long, and useless without the verifier that never leaves the app. This is the standard native-app trade and the reason PKCE ships at v1.
- **`uuid` as a randomness source** is correct today (v4 uses `getrandom`) but is an indirect guarantee. Task 1.2 pins it with a test asserting two calls differ and the verifier's length and alphabet.

## Migration Plan

1. Web app and API change lands and deploys to staging (`9_sop-sorcery`, separate change).
2. Recorder change lands behind no flag -- the login screen simply has a button and a fallback link.
3. Verified against staging (tasks 7.1-7.2), then the API deploys to production before the recorder is tagged.
4. Existing installs are unaffected: a stored refresh token means the login screen is never reached.
5. Next release removes the password form, the `login` command and `net_auth::sign_in`.

## Open Questions

- ~~Should `/desktop/authorize` confirm the sign-in with the user before redirecting?~~ **Decided, and in scope** in `9_sop-sorcery`/`desktop-browser-login`: every authorization is confirmed, including when the user is already signed in, because without it any local process can obtain a session with no user interaction at all. The recorder is unaffected either way, but this question must not be reopened from here -- dropping the prompt reinstates a silent local-capture path.
- Whether the fallback link should be hidden behind a keyboard chord rather than shown to everyone. Left visible for one release, on the grounds that a user who needs it is already locked out and in no state to discover a chord.
