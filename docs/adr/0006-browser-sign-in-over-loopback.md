# Browser sign-in over a loopback listener, with the password form kept

Desktop sign-in opens the web app in the user's browser and receives the code on a listener the
recorder binds on `127.0.0.1:<random port>`, with PKCE and a per-attempt `state`. A registered URL
scheme (`cogniclone://`) was the alternative; loopback needs no installer registration and works the
same in dev builds, and on Brave, Safari and Firefox the redirect to `127.0.0.1` was followed without
any prompt. The listener is about 200 hand-written lines over a TCP listener, not a web framework,
because its whole job is one request line.

The in-app password sign-in stays, with no removal date. Instead, a failed browser sign-in creates an
error report (under the user's report setting), and removing the form is a later decision on those
reports, not on a release count.

## Consequences

- Switching to the URL scheme means deleting `src-tauri/src/network/loopback.rs`, adding the
  deep-link and single-instance plugins and the scheme in `tauri.conf.json`, and feeding the same
  callback shape from the plugin. Nothing in `network/auth.rs`, the server or the UI changes; the web
  app already accepts both redirect shapes.
- The loopback connection is drained after it is answered; closing it with unread headers sends a
  reset, and the browser then drops the redirect it already read.
- A stale browser tab can still mint a code for an attempt the recorder gave up. The code is
  single-use, short-lived and bound to a verifier only the dead attempt held, so it is unusable.
