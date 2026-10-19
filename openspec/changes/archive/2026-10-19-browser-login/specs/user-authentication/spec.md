## Purpose

How a user proves who they are to the recorder: where credentials may be entered, what the application is allowed to accept back from outside itself, and what the user is told when that exchange fails, is cancelled, or is tampered with.

## ADDED Requirements

### Requirement: Credentials are entered in the user's browser

Signing in SHALL be performed in the user's default browser, on the account's own web login page. The recorder SHALL open that page itself and SHALL NOT present a credential form as the primary sign-in path.

#### Scenario: Starting a sign-in

- **WHEN** the user starts a sign-in from the login screen
- **THEN** the account's login page opens in the default browser
- **AND** the recorder window shows that it is waiting for the browser

#### Scenario: The user is already signed in on the web

- **WHEN** the browser already holds a valid web session for the account
- **THEN** the user is not asked for credentials again
- **AND** the recorder completes the sign-in without any credential entry

#### Scenario: A password manager fills the form

- **WHEN** the user's browser or password-manager extension fills the login form
- **THEN** the sign-in proceeds exactly as a typed one; the recorder is not involved in the entry

#### Scenario: The browser is signed in to a different account

- **WHEN** the browser's web session belongs to an account other than the one the user intends to use
- **THEN** the confirmation step in the browser names that account and offers to switch accounts before any code is issued
- **AND** after a sign-in, the recorder shows the email of the account it is signed in as

### Requirement: The recorder never handles the user's password

The recorder SHALL NOT read, store, transmit or hold in memory the user's account password as part of the browser sign-in path.

#### Scenario: Completing a browser sign-in

- **WHEN** a browser sign-in completes successfully
- **THEN** the only credential material the recorder has received is a single-use authorization code and the tokens exchanged for it

### Requirement: Only an authorization code obtained by the app itself is accepted

The recorder SHALL accept a sign-in callback only when it carries the exact `state` value minted for the sign-in attempt currently in progress. A callback with a missing, stale or mismatched `state` SHALL be rejected without being exchanged.

#### Scenario: A callback with a mismatched state

- **WHEN** a callback arrives whose `state` does not equal the pending attempt's `state`
- **THEN** no exchange is attempted
- **AND** the user remains signed out and is told the sign-in could not be verified

#### Scenario: A callback with no sign-in in progress

- **WHEN** a callback arrives while no sign-in attempt is pending
- **THEN** it is discarded and the session is unchanged

#### Scenario: A callback carrying an error instead of a code

- **WHEN** the callback carries an error rather than a code
- **THEN** the user is shown that the sign-in did not complete, and can retry

### Requirement: The authorization code is bound to the requesting application

The recorder SHALL generate a fresh code verifier for every sign-in attempt, send only its SHA-256 challenge to the browser, and present the verifier when exchanging the code. The verifier SHALL NOT leave the application by any other route, and SHALL NOT be reused between attempts.

#### Scenario: A code intercepted without the verifier

- **WHEN** an authorization code is presented for exchange without the matching verifier
- **THEN** no tokens are issued

#### Scenario: Two sign-in attempts in succession

- **WHEN** a sign-in is cancelled or times out and another is started
- **THEN** the second attempt uses a new verifier and a new `state`
- **AND** a code issued for the first attempt cannot be exchanged by the second

### Requirement: The callback is only accepted from the app's own redirect target

The redirect target the recorder asks the browser to return to SHALL be either a loopback address bound by the recorder for that attempt, or a URL scheme registered to the application. The recorder SHALL bind its loopback listener before opening the browser, SHALL bind it to the IPv4 loopback address only, and SHALL stop listening as soon as the attempt ends by any route.

#### Scenario: The listener is not reachable from the network

- **WHEN** a sign-in is in progress
- **THEN** the listener accepts connections only on the loopback interface

#### Scenario: The attempt ends

- **WHEN** the sign-in completes, is cancelled, fails or times out
- **THEN** the listener is closed and the port released

#### Scenario: An unrelated request reaches the listener

- **WHEN** a request arrives that is not a well-formed callback
- **THEN** it is rejected without affecting the pending attempt
- **AND** the recorder continues waiting for the callback

#### Scenario: A connection that sends nothing

- **WHEN** a connection is opened to the listener and carries no request
- **THEN** it does not consume or delay the pending attempt
- **AND** a callback arriving on a later connection still completes the sign-in

### Requirement: The browser shows the outcome the recorder reached

When the callback reaches the recorder through its loopback listener, the browser SHALL be answered only after the recorder has validated the callback and completed or failed the exchange, and the answer SHALL reflect that result. The browser SHALL NOT be told the sign-in succeeded when the recorder did not obtain a session. No token, code or account identifier SHALL appear in the URL the browser is sent to.

#### Scenario: A successful sign-in

- **WHEN** the exchange returns a session
- **THEN** the browser is sent to the web app's signed-in page
- **AND** the recorder is signed in

#### Scenario: The session cannot be stored

- **WHEN** the exchange returns a session but storing it in the operating system's credential store fails
- **THEN** the browser is sent to the failure page
- **AND** the recorder reports the sign-in as failed
- **AND** no credential is left stored, including when the store partially succeeded

#### Scenario: A sign-in that fails after the callback

- **WHEN** the callback's `state` does not match, or the exchange is refused, or the server does not support it
- **THEN** the browser is sent to the web app's failure page with a reason naming which of these happened
- **AND** the browser does not show that the sign-in succeeded

#### Scenario: The exchange does not answer

- **WHEN** the exchange has not completed within the recorder's exchange deadline after the callback arrives
- **THEN** the browser is sent to the failure page rather than left loading
- **AND** the recorder reports its own result when the exchange resolves

### Requirement: The recorder comes to the front after signing in

When a browser sign-in completes successfully, the recorder SHALL restore and focus its main window, so that the user, who is looking at the browser, is returned to the app.

#### Scenario: The recorder is behind the browser

- **WHEN** a browser sign-in completes while the recorder window is minimized or behind the browser
- **THEN** the recorder window is restored and brought to the front, or, where the operating system refuses to change focus, signals the user through its taskbar entry

### Requirement: A sign-in in progress can be abandoned

The recorder SHALL let the user cancel a sign-in at any point before it completes, whether it is waiting for the browser or exchanging the code it received. It SHALL abandon an attempt for which no callback arrives within five minutes, and SHALL abandon an exchange that has not returned within its own, much shorter deadline. In either case the user SHALL be returned to a login screen from which a new attempt can be started, and no credential from an abandoned attempt SHALL be stored.

#### Scenario: The user cancels

- **WHEN** the user cancels while the recorder is waiting for the browser
- **THEN** waiting stops immediately
- **AND** the login screen is usable again without restarting the app

#### Scenario: The exchange does not answer

- **WHEN** the browser callback has arrived and the exchange with the server does not complete
- **THEN** the attempt is abandoned within the same order of time as the user would wait for any other request, and the user is told the sign-in did not complete and can be retried
- **AND** no session is stored

#### Scenario: The user cancels after returning from the browser

- **WHEN** the user cancels after the browser callback has arrived but before the exchange has completed
- **THEN** the attempt ends as cancelled and the user is returned to a usable login screen
- **AND** a session arriving from that exchange afterwards is discarded rather than stored

#### Scenario: The user never finishes in the browser

- **WHEN** five minutes pass with no callback
- **THEN** the attempt is abandoned and the user is told the sign-in timed out and can be retried

#### Scenario: A late callback

- **WHEN** a callback arrives after its attempt was cancelled or timed out
- **THEN** it is ignored and does not sign the user in

#### Scenario: A second sign-in is started while one is waiting

- **WHEN** a new browser sign-in is started while an earlier one is still waiting
- **THEN** the earlier attempt is abandoned before the new one begins
- **AND** a callback belonging to the abandoned attempt is treated as a late callback, not as a tampered one

### Requirement: The desktop session is independent of the browser's

The session obtained by a browser sign-in SHALL be the recorder's own. Refreshing it SHALL NOT invalidate the browser's session, and ending the browser's session SHALL NOT invalidate the recorder's.

#### Scenario: Both clients refresh

- **WHEN** the recorder and the browser each refresh their session, in either order
- **THEN** both continue to work, and neither is signed out

#### Scenario: The user signs out in the browser

- **WHEN** the user signs out of the web application
- **THEN** the recorder's session remains valid

### Requirement: A server that cannot complete a browser sign-in says so

Where the server does not support the browser sign-in exchange, the recorder SHALL report that distinctly from a rejected credential, and SHALL point the user at the sign-in path that still works.

#### Scenario: The server predates the browser sign-in

- **WHEN** the exchange endpoint answers that it does not exist, or answers with a body that is not the expected JSON at all
- **THEN** the user is told the server does not support browser sign-in
- **AND** the message is distinguishable from a failed authentication
- **AND** the user is directed to the in-app credential form

#### Scenario: The unsupported server is recognised before the browser opens

- **WHEN** the user starts a browser sign-in against a server whose exchange endpoint does not exist
- **THEN** the recorder reports the unsupported server without opening the browser
- **AND** the user is not left waiting for a callback that a server without the endpoint can never produce

#### Scenario: Whether the server supports browser sign-in cannot be determined

- **WHEN** the recorder cannot reach the server, or gets no answer in time, while checking for the exchange endpoint
- **THEN** the browser sign-in proceeds as if the endpoint exists
- **AND** the user is NOT told the server does not support browser sign-in

#### Scenario: The exchange refuses an otherwise supported request

- **WHEN** the exchange endpoint exists and refuses the request -- an expired, replayed or unknown code, or a verifier that does not match
- **THEN** the user is told the sign-in did not complete and can be retried
- **AND** the message is NOT the "server does not support browser sign-in" message, because the browser path works on this server

### Requirement: A sign-in without a browser remains available for one release

While the browser path is the primary one, the login screen SHALL still offer a way to sign in by entering credentials in the app, so that a machine on which the browser handoff cannot work is not locked out of the product.

#### Scenario: The browser handoff cannot complete on this machine

- **WHEN** the browser does not open, or no callback can reach the recorder
- **THEN** the user can reach the in-app credential form from the login screen and sign in with it

### Requirement: A completed browser sign-in is indistinguishable from any other

Once tokens have been obtained, the recorder SHALL store and use them exactly as it does for any other sign-in: the refresh token in the operating system credential store, the access token in memory, and session restore and expiry unchanged.

#### Scenario: Restarting after a browser sign-in

- **WHEN** the app is restarted after a successful browser sign-in
- **THEN** the stored session is restored without any sign-in prompt

#### Scenario: The session expires

- **WHEN** the stored session can no longer be refreshed
- **THEN** the user is returned to the login screen with the existing expiry message

#### Scenario: Signing out

- **WHEN** the user signs out
- **THEN** the stored credentials are cleared as before, and the next sign-in starts a fresh browser attempt
