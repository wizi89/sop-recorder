## Why

Signing in to the recorder means typing an email and a password into a Tauri window (`src/components/LoginScreen.tsx`). That window is a system webview, not a browser: it has no extension host, so Dashlane, 1Password, Bitwarden and the browser's own password store cannot fill it, and never will. Every user types their password by hand on every fresh install, which pushes them towards short, reused, memorable passwords -- the app's own login form is the weakest credential surface the product has.

The fix is the handoff Postman, Figma and GitHub Desktop all use: the app opens the account's normal web login in the user's real browser, the user signs in there with whatever tooling they already trust, and the browser hands a short-lived code back to the app. It also removes a class of support problem -- the recorder no longer has its own password field to get wrong, and a user who is already signed in to the web app signs in to the recorder with one click and no typing.

This change also settles the mechanism question up front. A custom URL scheme (`cogniclone://`, what Postman uses) and a loopback redirect (`http://127.0.0.1:<port>/callback`, what RFC 8252 recommends) differ only in how the callback reaches the app. This change ships loopback, because the app owns no URL scheme today and adding one costs an OS registration, the `deep-link` plugin and the `single-instance` plugin -- but every other part is built so that the scheme can be added later without re-opening the server contract, the security design or the UI.

## What Changes

- **The recorder stops collecting passwords.** The email and password fields are removed from `LoginScreen`. The `login(email, password)` command is replaced by `login_via_browser()`, which opens the web app's login page in the default browser and waits for the callback. The recorder never sees, holds or transmits the user's password.
- **A one-time authorization code replaces the password round-trip.** The browser returns a code; the app exchanges it, with a PKCE verifier, for the `{access_token, refresh_token, email}` triple that `network/auth.rs` already understands. Everything downstream of that triple -- the keyring, `refresh_session`, the `auth:session_expired` event, `SessionCache` -- is unchanged.
- **PKCE and a CSRF `state` are part of the first release, not a later hardening pass.** They are the work that would otherwise have to be redone across all three components -- recorder, web app and API -- in one coordinated release, on the day the custom scheme is added. See the design's "What carries over".
- **The callback listener is the only mechanism-specific code.** It binds `127.0.0.1:0`, accepts connections until a well-formed callback arrives or the attempt ends -- ignoring the speculative connections browsers open and carry no request on -- and is deleted when the scheme lands. Roughly 60 lines; the rest of the flow parses and validates a `Url` without knowing where it came from.
- **The browser tab tells the truth.** The listener answers the browser only once the recorder has validated the callback and exchanged the code, redirecting to a web-app page for success or for the specific failure. The web app's pages carry the branding, the signed-in account, "Not you? Switch account" and help. The browser never shows success for a sign-in the recorder rejected.
- **The recorder comes to the front when sign-in completes**, instead of staying hidden behind the browser the user is looking at.
- **A sign-in in progress can be cancelled and times out, in both phases.** The window shows that it is waiting for the browser, offers a cancel, and gives up after five minutes with a message rather than hanging -- and the cancel and the exchange deadline stay live across the code exchange too, so a backend that accepts the connection and never answers cannot strand the app or let it sign in behind a login screen the user was already sent back to.
- **The password form survives one release as a fallback**, behind a "sign in without a browser" link, so a machine where the browser cannot reach the loopback listener is not locked out of the product. Its removal is task 8.1 of the *next* change. The trigger recorded there is support silence, not a measured signal: nothing in this change reports a failed browser sign-in anywhere but the local log, so the EDR and loopback-blocking risks are checked once by hand at release (task 7.1b) and never again. If that is not good enough when the time comes, the follow-up's first task is to add the signal rather than to remove the fallback.

Non-goals:

- **Shipping the custom URL scheme.** Deferred deliberately, with the seam documented. It becomes worth doing when the product wants `cogniclone://` links on the web for their own sake (open a recording, join a workspace); login then moves across for the cost of one function body.
- **Third-party identity providers.** Google/Microsoft sign-in is a Supabase configuration plus web app work, and the browser handoff is its precondition, not its implementation. Out of scope here.
- **Changing session storage, refresh, or expiry.** `keyring`, the `SERVICE_NAME` split between dev and release builds, and the refresh path are untouched.
- **The web app and API changes themselves.** They are server-side and need their own OpenSpec change; that change, `desktop-browser-login`, is authoritative for the server side; design.md's "Server contract" section states only the surface the recorder touches, and this change depends on that one shipping first.

## Capabilities

### New Capabilities

- `user-authentication`: how a user proves who they are to the recorder -- where credentials are entered, what the app is allowed to accept back, and what happens when the exchange fails, is cancelled or is tampered with.

### Modified Capabilities

None.

## Impact

Affected code:

- `src-tauri/src/network/auth.rs` -- `begin()`, `parse_callback()`, `complete()`; `sign_in` retained for the fallback, removed next release.
- `src-tauri/src/commands/auth.rs` -- `login_via_browser`, `cancel_browser_login`, pending-login state alongside `SessionCache`.
- `src-tauri/src/network/loopback.rs` (new) -- the callback listener. The file that later disappears.
- `src-tauri/src/lib.rs:166` -- command registration.
- `src-tauri/Cargo.toml` -- `sha2`, `base64`.
- `src/components/LoginScreen.tsx`, `src/hooks/useAuth.ts`, `src/lib/tauri.ts`, `src/i18n/de.ts` -- one button, a waiting state, a cancel, the fallback link and its copy.
- `src/test/useAuth.test.ts`, `src/test/components.test.tsx` -- the `login(email, password)` call shape changes.

Compatibility:

- **The server must ship first.** A recorder built from this change cannot sign in against an API without `/auth/desktop/exchange`. The fallback path keeps working against the old API, and the recorder names that case rather than reporting a generic authentication failure (task 7.7), so the failure is recoverable, not fatal -- but the release order is not optional.
- Stored credentials are untouched: an existing install with a valid refresh token in the keyring never sees the login screen at all.
- `SessionState`, the `auth:session_expired` event and every authenticated request keep their current shape.

Dependencies added: `sha2` (PKCE challenge), `base64` (base64url encoding). No new Tauri plugins -- the browser is opened with `tauri-plugin-opener`, already in use at `LoginScreen.tsx:69` for "forgot password".
