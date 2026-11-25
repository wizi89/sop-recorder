use base64::Engine;
use keyring::Entry;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::config;

/// Keychain service the credentials live under.
///
/// Dev builds deliberately use a separate service from the shipped app. The
/// macOS Keychain grants access to a *code identity*, and a dev build's identity
/// is not the shipped app's -- so a dev build reading the release app's items
/// is an untrusted caller, and macOS prompts for the login password on every
/// read, twice over (once for each item). Giving dev its own items means the
/// dev binary creates them itself, and the creating process is on the item's
/// ACL from the start, so it is never asked again.
///
/// The release name is untouched, so a developer's real login survives, and a
/// dev build can never spend a release token by accident.
#[cfg(debug_assertions)]
const SERVICE_NAME: &str = "cogniclone-dev";
#[cfg(not(debug_assertions))]
const SERVICE_NAME: &str = "cogniclone";

const LEGACY_SERVICE_NAME: &str = "sop-sorcery";

/// Migrate credentials from the old "sop-sorcery" keyring to "cogniclone".
/// Safe to call multiple times -- no-ops once old entries are gone.
pub fn migrate_keyring() {
    for key in &["refresh-token", "email"] {
        let old = match Entry::new(LEGACY_SERVICE_NAME, key) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let value = match old.get_password() {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Write to new service name, then delete old entry
        if let Ok(new) = Entry::new(SERVICE_NAME, key) {
            let _ = new.set_password(&value);
        }
        let _ = old.delete_credential();
    }
}

/// Pure mapping from upload target to compile-time default URL. Hermetic:
/// does not consult environment or config file. Used by tests; callers should
/// prefer `api_url_for_target`, which also honors runtime overrides.
fn api_url_default(upload_target: Option<&str>) -> &'static str {
    match upload_target {
        Some("Local") => config::API_URL_DEV,
        Some("Staging") => config::API_URL_STAGING,
        _ => config::API_URL_PROD,
    }
}

pub fn api_url_for_target(upload_target: Option<&str>) -> &'static str {
    if let Some(url) = crate::runtime_config::runtime().api_url.as_deref() {
        return url;
    }
    api_url_default(upload_target)
}

#[derive(Debug, Deserialize)]
struct SignInResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    email: Option<String>,
    message: Option<String>,
    error: Option<String>,
}

pub struct AuthSession {
    pub access_token: String,
    pub email: Option<String>,
}

/// Sign in with email/password via the FastAPI server's /auth/signin endpoint.
pub async fn sign_in(email: &str, password: &str, api_base: &str) -> Result<AuthSession, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/auth/signin", api_base);

    let res = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    let data: SignInResponse = res.json().await.map_err(|e| e.to_string())?;

    let access_token = data.access_token.ok_or_else(|| {
        data.message
            .or(data.error)
            .unwrap_or_else(|| "Anmeldung fehlgeschlagen".into())
    })?;

    let refresh_token = data.refresh_token.unwrap_or_default();

    // Store refresh token + email in keyring
    if !refresh_token.is_empty() {
        keyring_save("refresh-token", &refresh_token)?;
    }
    if let Some(ref email) = data.email {
        keyring_save("email", email)?;
    }

    Ok(AuthSession {
        access_token,
        email: data.email,
    })
}

// -- Browser sign-in (OpenSpec: browser-login) --------------------------------
//
// The user's credentials are entered in their real browser, where their
// password manager works. The browser hands back a one-time code, which the
// recorder exchanges -- with a PKCE verifier that never leaves this process --
// for the same {access_token, refresh_token, email} triple `sign_in` returns.
//
// Everything here is pure except `complete`, and `complete` deliberately
// persists nothing: see its doc comment.

/// One sign-in attempt's secrets. Lives in app state for the attempt's
/// lifetime and nowhere else.
pub struct PendingLogin {
    pub state: String,
    pub verifier: String,
}

/// Mint `state` and a PKCE verifier, and return the challenge to send.
///
/// Both values are two v4 UUIDs' worth of randomness (122 bits apiece, from
/// `getrandom`). The hex verifier is 64 characters, inside PKCE's 43-128.
pub fn begin() -> (PendingLogin, String) {
    fn two_uuids() -> String {
        format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        )
    }

    let verifier = two_uuids();
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(verifier.as_bytes()));

    (
        PendingLogin {
            state: two_uuids(),
            verifier,
        },
        challenge,
    )
}

/// Why a callback did not yield a code.
///
/// An enum rather than a `String`: the decline has to survive as a decline.
/// Collapsed into prose, only string matching could recover it, nobody writes
/// that match, and the user who pressed "cancel" on the consent screen is
/// shown a generic failure instead.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CallbackError {
    /// The user said no on the consent screen (`error=access_denied`).
    Denied,
    /// The server could not mint a code (`error=server_error`).
    ServerError,
    /// Anything else: an unknown `error` value, or a missing `code`/`state`.
    Malformed,
}

/// Pull `(code, state)` out of a callback URL.
///
/// Mechanism-agnostic on purpose: `url::Url` parses
/// `http://127.0.0.1:53124/callback?code=..&state=..` and
/// `cogniclone://auth/callback?code=..&state=..` into the same query pairs, so
/// this function is unaffected by the deep-link switch.
pub fn parse_callback(url: &Url) -> Result<(String, String), CallbackError> {
    let mut code = None;
    let mut state = None;
    let mut error = None;

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(error) = error {
        return Err(match error.as_str() {
            "access_denied" => CallbackError::Denied,
            "server_error" => CallbackError::ServerError,
            _ => CallbackError::Malformed,
        });
    }

    match (code, state) {
        (Some(code), Some(state)) if !code.is_empty() && !state.is_empty() => Ok((code, state)),
        _ => Err(CallbackError::Malformed),
    }
}

/// Compare two strings without an early return on the first difference.
///
/// The `state` check is the only comparison in this flow against a secret the
/// other side chose the timing of.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// What the exchange returns. Exists only so the result can cross the
/// `select!` boundary without having been persisted on the way.
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub email: Option<String>,
}

/// Exchange the code for a session. **Persists nothing.**
///
/// This is the one place that must not copy `sign_in`, which saves to the
/// keyring before it returns. `complete` runs inside a cancellable `select!`,
/// so a save here would let a cancel that wins the race leave a stored refresh
/// token behind a UI that said the sign-in was cancelled -- and
/// `refresh_session` would restore it silently on the next launch. The command
/// writes the keyring after the `select!` has resolved in the exchange's
/// favour.
///
/// The error handling is also not `sign_in`'s. `sign_in` never inspects the
/// status: it parses the body and falls back to "Anmeldung fehlgeschlagen", so
/// a 404 `{"detail":"Not Found"}` from an API without this endpoint, or an
/// HTML error page from a proxy, would surface as an ordinary failed login.
/// Here that case has to be distinguishable, because it is the one failure
/// whose answer is the fallback form rather than "try again".
pub const UNSUPPORTED_SERVER: &str = "browser-login-unsupported";

/// The exchange's own deadline. The middle term of the agreed ordering:
/// server upstream bound (10) < this (15) < the browser `finish` bound (20).
/// `sign_in`'s client has no timeout at all, so copying it wholesale would
/// leave a hung backend unbounded.
pub const EXCHANGE_TIMEOUT_SECS: u64 = 15;

pub async fn complete(p: &PendingLogin, code: &str, api_base: &str) -> Result<Tokens, String> {
    complete_within(
        p,
        code,
        api_base,
        std::time::Duration::from_secs(EXCHANGE_TIMEOUT_SECS),
    )
    .await
}

/// `complete` with the deadline as a parameter, so a test can assert the
/// hung-backend case without waiting out the shipped one.
async fn complete_within(
    p: &PendingLogin,
    code: &str,
    api_base: &str,
    timeout: std::time::Duration,
) -> Result<Tokens, String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .post(format!("{}/auth/desktop/exchange", api_base))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "code": code, "code_verifier": p.verifier }))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Die Anmeldung hat zu lange gedauert.".to_string()
            } else {
                format!("Connection failed: {}", e)
            }
        })?;

    let status = res.status();
    let body = res.text().await.map_err(|e| e.to_string())?;
    if lacks_exchange(status, &body) {
        return Err(UNSUPPORTED_SERVER.to_string());
    }
    let data: SignInResponse = serde_json::from_str(&body).expect("checked by lacks_exchange");

    let access_token = data.access_token.ok_or_else(|| {
        data.message
            .or(data.error)
            .unwrap_or_else(|| "Anmeldung fehlgeschlagen".into())
    })?;

    Ok(Tokens {
        access_token,
        refresh_token: data.refresh_token.unwrap_or_default(),
        email: data.email,
    })
}

/// Whether a response to `/auth/desktop/exchange` says the endpoint is not
/// there. 404 or 405 is an API deployed before it; a body that is not JSON at
/// all is an HTML error page from a proxy, or a plain-text gateway error --
/// indistinguishable from an API that predates the endpoint, and the same
/// answer helps the user either way. Shared by `complete` and
/// `probe_exchange`, so the two cannot disagree about what "unsupported" is.
fn lacks_exchange(status: reqwest::StatusCode, body: &str) -> bool {
    status.as_u16() == 404
        || status.as_u16() == 405
        || serde_json::from_str::<SignInResponse>(body).is_err()
}

/// The probe's deadline. Short, because the user has just clicked and is
/// waiting for a browser to open; an inconclusive probe is not a failure.
pub const PROBE_TIMEOUT_SECS: u64 = 5;

/// Ask the API whether it has the exchange endpoint, before the browser opens.
///
/// Without this the old-server case is only detected at the exchange, which
/// never happens: the web app predating the change has no `/desktop/authorize`
/// either, so the browser lands on a 404 page, no callback arrives, and the
/// attempt ends in a timeout five minutes later instead of pointing the user
/// at the password form. That is the state everyone is in if a recorder
/// release ships before the server, or the server is rolled back.
///
/// An empty body is safe to send: there is no code to consume, and a server
/// that has the endpoint refuses it with a JSON `invalid_code`. Only a
/// definite "not there" is reported. A network failure or timeout returns
/// `Ok` and the attempt goes ahead as it did before, because nothing about
/// the probe failing says which way the exchange will go.
pub async fn probe_exchange(api_base: &str) -> Result<(), String> {
    probe_exchange_within(api_base, std::time::Duration::from_secs(PROBE_TIMEOUT_SECS)).await
}

async fn probe_exchange_within(api_base: &str, timeout: std::time::Duration) -> Result<(), String> {
    let Ok(client) = reqwest::Client::builder().timeout(timeout).build() else {
        return Ok(());
    };
    let res = client
        .post(format!("{}/auth/desktop/exchange", api_base))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await;
    let Ok(res) = res else {
        return Ok(());
    };
    let status = res.status();
    let Ok(body) = res.text().await else {
        return Ok(());
    };
    if lacks_exchange(status, &body) {
        Err(UNSUPPORTED_SERVER.to_string())
    } else {
        Ok(())
    }
}

/// Persist what the exchange returned, exactly as `sign_in` does.
///
/// Called by the command after the `select!`, never inside it. `keyring_save`
/// runs twice, and a failure of the second would otherwise leave the refresh
/// token written while the command reports failure -- so a partial failure
/// clears both entries.
pub fn save_session(tokens: &Tokens) -> Result<AuthSession, String> {
    if !tokens.refresh_token.is_empty() {
        keyring_save("refresh-token", &tokens.refresh_token).inspect_err(|_| sign_out())?;
    }
    if let Some(ref email) = tokens.email {
        // The refresh token is already written at this point, and
        // `refresh_session` reads only that entry -- it would otherwise
        // restore on the next launch the session the user was just told had
        // failed.
        keyring_save("email", email).inspect_err(|_| sign_out())?;
    }

    Ok(AuthSession {
        access_token: tokens.access_token.clone(),
        email: tokens.email.clone(),
    })
}

/// Refresh session using stored refresh token via /auth/refresh.
pub async fn refresh_session(api_base: &str) -> Result<Option<AuthSession>, String> {
    let refresh_token = match keyring_load("refresh-token").map_err(|e| e.to_string())? {
        Some(t) => t,
        None => return Ok(None),
    };

    let client = reqwest::Client::new();
    let url = format!("{}/auth/refresh", api_base);

    let res = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .map_err(|e| format!("Refresh failed: {}", e))?;

    let status = res.status().as_u16();
    if token_rejected(status) {
        let _ = keyring_clear("refresh-token");
        return Ok(None);
    }
    if !res.status().is_success() {
        // A deploy, an overloaded server or a proxy in between: the token may
        // be fine, and deleting it would sign the user out over a hiccup.
        return Err(format!("Refresh failed: server answered {}", status));
    }

    let data: SignInResponse = res.json().await.map_err(|e| e.to_string())?;

    let access_token = match data.access_token {
        Some(t) => t,
        None => {
            let _ = keyring_clear("refresh-token");
            return Ok(None);
        }
    };

    if let Some(ref rt) = data.refresh_token {
        keyring_save("refresh-token", rt)?;
    }
    if let Some(ref email) = data.email {
        keyring_save("email", email)?;
    }

    Ok(Some(AuthSession {
        access_token,
        email: data.email,
    }))
}

/// The answers that mean the refresh token itself is no good. Anything else --
/// 5xx, 429, a proxy's 404 -- says nothing about the token.
fn token_rejected(status: u16) -> bool {
    matches!(status, 400 | 401 | 403)
}

/// Sign out: clear all stored credentials.
pub fn sign_out() {
    let _ = keyring_clear("refresh-token");
    let _ = keyring_clear("email");
}

// -- Keyring helpers --

pub fn keyring_save(key: &str, value: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, key).map_err(|e| e.to_string())?;
    entry.set_password(value).map_err(|e| e.to_string())
}

pub fn keyring_load(key: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE_NAME, key).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn keyring_clear(key: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, key).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests;
