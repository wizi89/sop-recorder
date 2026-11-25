use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;
use tauri_plugin_store::StoreExt;
use tokio_util::sync::CancellationToken;

use crate::network::auth as net_auth;
use crate::network::loopback::{self, Outcome};

#[derive(Serialize, Clone)]
pub struct SessionState {
    pub logged_in: bool,
    pub email: Option<String>,
}

/// In-memory session cache (access token + email).
pub struct SessionCache {
    pub access_token: Mutex<Option<String>>,
    pub email: Mutex<Option<String>>,
}

impl Default for SessionCache {
    fn default() -> Self {
        Self {
            access_token: Mutex::new(None),
            email: Mutex::new(None),
        }
    }
}

pub fn get_api_base(app: &tauri::AppHandle) -> &'static str {
    let target = app.store("settings.json").ok().and_then(|store| {
        store
            .get("upload_target")
            .and_then(|v| v.as_str().map(String::from))
    });
    net_auth::api_url_for_target(target.as_deref())
}

#[tauri::command]
pub async fn login(
    email: String,
    password: String,
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
) -> Result<SessionState, String> {
    let api_base = get_api_base(&app);
    let auth = net_auth::sign_in(&email, &password, api_base).await?;

    *session
        .access_token
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(auth.access_token);
    *session.email.lock().unwrap_or_else(|e| e.into_inner()) = auth.email.clone();

    Ok(SessionState {
        logged_in: true,
        email: auth.email,
    })
}

#[tauri::command]
pub async fn logout(session: State<'_, SessionCache>) -> Result<(), String> {
    net_auth::sign_out();
    *session
        .access_token
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    *session.email.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}

#[tauri::command]
pub async fn refresh_session(
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
) -> Result<SessionState, String> {
    let api_base = get_api_base(&app);
    match net_auth::refresh_session(api_base).await? {
        Some(auth) => {
            *session
                .access_token
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(auth.access_token);
            *session.email.lock().unwrap_or_else(|e| e.into_inner()) = auth.email.clone();
            Ok(SessionState {
                logged_in: true,
                email: auth.email,
            })
        }
        None => Ok(SessionState {
            logged_in: false,
            email: None,
        }),
    }
}

#[tauri::command]
pub async fn get_session_state(session: State<'_, SessionCache>) -> Result<SessionState, String> {
    let token = session
        .access_token
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let email = session
        .email
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(SessionState {
        logged_in: token.is_some(),
        email,
    })
}

// -- Browser sign-in (OpenSpec: browser-login) --------------------------------

/// How long the user has to finish in the browser.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// How long the code exchange may take. The middle term of the agreed
/// ordering: server upstream bound (10) < this (15) < browser `finish` (20).
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(15);

/// The cancel handle of the attempt in flight, if there is one.
///
/// A token **created with the attempt and replaced, not re-signalled, by the
/// next one**. A single long-lived `tokio::sync::Notify` does not work here:
/// `notify_one()` with no waiter stores a permit, so the unconditional cancel
/// that supersedes a previous attempt would make every attempt after the first
/// cancel itself on its first poll, and browser sign-in would be dead until
/// the app restarted.
#[derive(Default)]
pub struct PendingBrowserLogin {
    /// The attempt in flight, as (id, token). The id exists because
    /// `CancellationToken` is not comparable, and `release` must not clear a
    /// *newer* attempt's token when a superseded attempt finishes last.
    slot: Mutex<Option<(u64, CancellationToken)>>,
    next_id: Mutex<u64>,
}

impl PendingBrowserLogin {
    /// Cancel whatever attempt is in flight and install this attempt's token.
    /// Returns the attempt's id, for `release`.
    fn install(&self, token: CancellationToken) -> u64 {
        let id = {
            let mut next = self.next_id.lock().unwrap_or_else(|e| e.into_inner());
            *next += 1;
            *next
        };
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, previous)) = slot.replace((id, token)) {
            // Supersede: cancel the *previous* attempt through its own token,
            // which drops its listener. A second attempt's callback must never
            // be checked against a newer attempt's `state` -- the user would
            // be shown the tampering message for a sign-in nobody tampered
            // with.
            previous.cancel();
        }
        id
    }

    /// Cancel the attempt in flight. A no-op once it has ended, because its
    /// token is gone from state by then.
    fn cancel(&self) {
        if let Some((_, token)) = self.slot.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            token.cancel();
        }
    }

    /// Clear the slot if it still holds this attempt.
    fn release(&self, id: u64) {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref().is_some_and(|(current, _)| *current == id) {
            *slot = None;
        }
    }
}

/// Map a failure to the token the browser is shown.
fn outcome_for(error: &str) -> Outcome {
    if error == net_auth::UNSUPPORTED_SERVER {
        Outcome::Unsupported
    } else {
        Outcome::Error
    }
}

#[tauri::command]
pub async fn login_via_browser(
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
    pending: State<'_, PendingBrowserLogin>,
) -> Result<SessionState, String> {
    let api_base = get_api_base(&app);
    let webapp = get_webapp_base(&app);

    // Before anything is bound or opened: against an API without the
    // endpoint the browser would only find a 404 page and the attempt would
    // run into its timeout. See `probe_exchange`.
    if let Err(e) = net_auth::probe_exchange(api_base).await {
        log::info!("Browser sign-in: server does not support it");
        return Err(e);
    }

    let (attempt, challenge) = net_auth::begin();

    let cancel = CancellationToken::new();
    let attempt_id = pending.install(cancel.clone());

    let callback = match loopback::start_loopback(&webapp).await {
        Ok(callback) => callback,
        // Installed before the bind so a cancel arriving during it is not
        // lost; that means the slot has to be cleared by hand when the bind
        // is what fails, exactly as the `open_url` arm below does.
        Err(e) => {
            pending.release(attempt_id);
            return Err(e);
        }
    };
    log::info!(
        "Browser sign-in: listening on {}",
        callback
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "127.0.0.1:?".into())
    );

    let authorize = format!(
        "{}/desktop/authorize?redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        webapp.trim_end_matches('/'),
        urlencode(&callback.redirect_uri),
        urlencode(&attempt.state),
        urlencode(&challenge),
    );

    if let Err(e) = tauri_plugin_opener::open_url(&authorize, None::<&str>) {
        pending.release(attempt_id);
        return Err(format!("Der Browser konnte nicht geöffnet werden: {}", e));
    }
    log::info!("Browser sign-in: browser opened");

    let (url, reply) = match callback.recv(CALLBACK_TIMEOUT, &cancel).await {
        Ok(pair) => pair,
        Err(e) => {
            // No connection is waiting at this point, so there is nothing to
            // answer: the browser is still on the web app's own pages.
            pending.release(attempt_id);
            log::info!("Browser sign-in: no callback ({})", e);
            return Err(e);
        }
    };
    log::info!("Browser sign-in: callback received");

    // Cloned so the attempt itself can move into the exchange future.
    let expected_state = attempt.state.clone();
    let outcome = resolve_attempt(
        &url,
        &expected_state,
        &cancel,
        EXCHANGE_TIMEOUT,
        |code: String| async move { net_auth::complete(&attempt, &code, api_base).await },
        net_auth::save_session,
    )
    .await;

    let shown = match &outcome {
        Ok(_) => Outcome::Success,
        Err(ExchangeError::Cancelled) => Outcome::Error,
        Err(ExchangeError::Callback(net_auth::CallbackError::Denied)) => Outcome::Denied,
        Err(ExchangeError::Callback(net_auth::CallbackError::ServerError)) => Outcome::Error,
        Err(ExchangeError::Callback(net_auth::CallbackError::Malformed)) => Outcome::Verification,
        Err(ExchangeError::Exchange(e)) => outcome_for(e),
    };
    reply.finish(&shown).await;

    pending.release(attempt_id);

    let auth = match outcome {
        Ok(auth) => auth,
        Err(e) => {
            log::info!("Browser sign-in failed: {:?}", shown);
            return Err(e.message());
        }
    };
    log::info!("Browser sign-in: completed");

    *session
        .access_token
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(auth.access_token);
    *session.email.lock().unwrap_or_else(|e| e.into_inner()) = auth.email.clone();

    // The user is in the browser at this moment, so the recorder is behind it.
    super::window::bring_to_front(&app, "main");

    Ok(SessionState {
        logged_in: true,
        email: auth.email,
    })
}

#[tauri::command]
pub async fn cancel_browser_login(pending: State<'_, PendingBrowserLogin>) -> Result<(), String> {
    log::info!("Browser sign-in: cancelled by the user");
    pending.cancel();
    Ok(())
}

/// Validate the callback, run the exchange, and decide the outcome.
///
/// Separated from the command because this is the part worth testing: it is
/// the sequencing the design is most specific about, and all of it is wrong in
/// ways that only show up under a race.
///
/// `exchange` and `save` are parameters so a test can stub a backend that
/// never answers and a credential store that fails.
async fn resolve_attempt<F, Fut, S>(
    url: &url::Url,
    expected_state: &str,
    cancel: &CancellationToken,
    deadline: Duration,
    exchange: F,
    save: S,
) -> Result<net_auth::AuthSession, ExchangeError>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<net_auth::Tokens, String>>,
    S: FnOnce(&net_auth::Tokens) -> Result<net_auth::AuthSession, String>,
{
    // The exchange is inside the `select!`, not awaited bare. Without this a
    // backend that accepts the connection and never answers would leave the
    // user on the waiting screen indefinitely, and a cancel would signal
    // nobody -- the UI would return to the login screen while the command
    // later succeeded, wrote the keyring and raised the window.
    let result = tokio::select! {
        r = async {
            let (code, state) =
                net_auth::parse_callback(url).map_err(ExchangeError::Callback)?;
            if !net_auth::constant_time_eq(&state, expected_state) {
                return Err(ExchangeError::Callback(net_auth::CallbackError::Malformed));
            }
            exchange(code).await.map_err(ExchangeError::Exchange)
        } => r,
        _ = tokio::time::sleep(deadline) => {
            Err(ExchangeError::Exchange("Die Anmeldung hat zu lange gedauert.".into()))
        }
        _ = cancel.cancelled() => Err(ExchangeError::Cancelled),
    };

    // One place decides the outcome, outside the cancellable future, before
    // the tab is answered.
    match result {
        // `select!` chooses at random among branches ready in the same wakeup,
        // so "the exchange branch won" is not evidence that no cancel arrived.
        // Without this re-check a cancel landing in that window would leave a
        // refresh token in the keyring behind a UI that said cancelled, and
        // `refresh_session` would restore it on the next launch.
        Ok(_) if cancel.is_cancelled() => Err(ExchangeError::Cancelled),
        // The keyring write happens here, before `finish`, so that a failed
        // save reaches the browser as reason=error rather than sending the tab
        // to the signed-in page for a sign-in the app is about to report as
        // failed. It is a local credential-store call of a few milliseconds
        // against a 20-second budget.
        Ok(tokens) => save(&tokens).map_err(ExchangeError::Exchange),
        Err(e) => Err(e),
    }
}

/// Why an attempt did not produce a session. Kept apart from a bare `String`
/// so the browser can be shown the right reason token.
#[derive(Debug)]
enum ExchangeError {
    Callback(net_auth::CallbackError),
    Exchange(String),
    Cancelled,
}

impl ExchangeError {
    fn message(self) -> String {
        match self {
            ExchangeError::Cancelled => "Die Anmeldung wurde abgebrochen.".into(),
            ExchangeError::Callback(net_auth::CallbackError::Denied) => {
                "browser-login-denied".into()
            }
            ExchangeError::Callback(_) => "browser-login-verification".into(),
            ExchangeError::Exchange(e) => e,
        }
    }
}

/// The same store lookup `get_api_base` does, for the web app's own URL.
fn get_webapp_base(app: &tauri::AppHandle) -> String {
    let target = app.store("settings.json").ok().and_then(|store| {
        store
            .get("upload_target")
            .and_then(|v| v.as_str().map(String::from))
    });
    crate::config::webapp_url_for_target(target.as_deref()).to_string()
}

/// Percent-encode a query parameter value. `url`'s own serializer is for whole
/// URLs; this is one value, and the set to escape is small and fixed.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

#[cfg(test)]
mod tests;
