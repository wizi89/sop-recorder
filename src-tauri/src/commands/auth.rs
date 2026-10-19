use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Manager, State};
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

    *session.access_token.lock().unwrap() = Some(auth.access_token);
    *session.email.lock().unwrap() = auth.email.clone();

    Ok(SessionState {
        logged_in: true,
        email: auth.email,
    })
}

#[tauri::command]
pub async fn logout(session: State<'_, SessionCache>) -> Result<(), String> {
    net_auth::sign_out();
    *session.access_token.lock().unwrap() = None;
    *session.email.lock().unwrap() = None;
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
            *session.access_token.lock().unwrap() = Some(auth.access_token);
            *session.email.lock().unwrap() = auth.email.clone();
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
    let token = session.access_token.lock().unwrap().clone();
    let email = session.email.lock().unwrap().clone();
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
            let mut next = self.next_id.lock().unwrap();
            *next += 1;
            *next
        };
        let mut slot = self.slot.lock().unwrap();
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
        if let Some((_, token)) = self.slot.lock().unwrap().as_ref() {
            token.cancel();
        }
    }

    /// Clear the slot if it still holds this attempt.
    fn release(&self, id: u64) {
        let mut slot = self.slot.lock().unwrap();
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

    *session.access_token.lock().unwrap() = Some(auth.access_token);
    *session.email.lock().unwrap() = auth.email.clone();

    bring_to_front(&app);

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

/// The user is in the browser at this moment, so the recorder is behind it.
fn bring_to_front(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        // On Windows, foreground-lock rules may turn this into a flashing
        // taskbar button rather than a raised window. Accepted: it still
        // points the user at the app.
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_escapes_what_a_query_value_must_not_carry() {
        assert_eq!(
            urlencode("http://127.0.0.1:53124/callback"),
            "http%3A%2F%2F127.0.0.1%3A53124%2Fcallback"
        );
        assert_eq!(urlencode("s=&?+"), "s%3D%26%3F%2B");
        assert_eq!(urlencode("abc-_.~123"), "abc-_.~123");
    }

    #[test]
    fn an_unsupported_server_gets_its_own_reason_token() {
        assert_eq!(
            outcome_for(net_auth::UNSUPPORTED_SERVER),
            Outcome::Unsupported
        );
        assert_eq!(outcome_for("anything else"), Outcome::Error);
    }

    #[test]
    fn installing_a_second_attempt_cancels_the_first() {
        let pending = PendingBrowserLogin::default();
        let first = CancellationToken::new();
        let second = CancellationToken::new();

        pending.install(first.clone());
        assert!(!first.is_cancelled());

        pending.install(second.clone());
        assert!(
            first.is_cancelled(),
            "the superseded attempt was not cancelled"
        );
        assert!(!second.is_cancelled(), "the new attempt cancelled itself");
    }

    #[test]
    fn a_retry_after_every_ending_starts_uncancelled() {
        // The test that fails if a shared Notify is used: notify_one() with no
        // waiter stores a permit, and the next attempt would cancel itself.
        let pending = PendingBrowserLogin::default();

        // ended by a cancel
        let first = CancellationToken::new();
        let first_id = pending.install(first.clone());
        pending.cancel();
        assert!(first.is_cancelled());
        pending.release(first_id);

        let second = CancellationToken::new();
        let second_id = pending.install(second.clone());
        assert!(
            !second.is_cancelled(),
            "retry after a cancel cancelled itself"
        );

        // ended by a timeout (the attempt releases its own token)
        pending.release(second_id);
        let third = CancellationToken::new();
        let third_id = pending.install(third.clone());
        assert!(
            !third.is_cancelled(),
            "retry after a timeout cancelled itself"
        );

        // ended by a failed exchange
        pending.release(third_id);
        let fourth = CancellationToken::new();
        pending.install(fourth.clone());
        assert!(
            !fourth.is_cancelled(),
            "retry after a failed exchange cancelled itself"
        );
    }

    #[test]
    fn cancelling_after_an_attempt_ended_is_a_no_op() {
        let pending = PendingBrowserLogin::default();
        let token = CancellationToken::new();
        let id = pending.install(token.clone());
        pending.release(id);

        pending.cancel(); // nothing in flight

        let next = CancellationToken::new();
        pending.install(next.clone());
        assert!(!next.is_cancelled());
    }

    // -- resolve_attempt: the sequencing (5.1d, 5.1g, 5.1h, 5.1i) ----------

    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    fn callback(code: &str, state: &str) -> url::Url {
        url::Url::parse(&format!(
            "http://127.0.0.1:1/callback?code={}&state={}",
            code, state
        ))
        .unwrap()
    }

    fn tokens() -> net_auth::Tokens {
        net_auth::Tokens {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            email: Some("user@example.test".into()),
        }
    }

    /// A save that records what it was asked to store, standing in for the
    /// keyring.
    fn recording_save(
        store: Arc<Mutex<Vec<String>>>,
        fail_after: Option<usize>,
    ) -> impl FnOnce(&net_auth::Tokens) -> Result<net_auth::AuthSession, String> {
        move |t: &net_auth::Tokens| {
            // Mirrors `save_session`: two writes in order, and a failure of
            // either clears both.
            let writes = ["refresh-token", "email"];
            for (i, key) in writes.iter().enumerate() {
                if Some(i) == fail_after {
                    store.lock().unwrap().clear(); // sign_out()
                    return Err("credential store failed".into());
                }
                store.lock().unwrap().push((*key).to_string());
            }
            Ok(net_auth::AuthSession {
                access_token: t.access_token.clone(),
                email: t.email.clone(),
            })
        }
    }

    #[tokio::test]
    async fn the_happy_path_saves_and_returns_a_session() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            |code| {
                assert_eq!(code, "abc");
                async { Ok(tokens()) }
            },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(got.is_ok());
        assert_eq!(*store.lock().unwrap(), vec!["refresh-token", "email"]);
    }

    #[tokio::test]
    async fn a_wrong_state_fails_verification_and_never_exchanges() {
        let called = Arc::new(AtomicBool::new(false));
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();
        let flag = called.clone();

        let got = resolve_attempt(
            &callback("abc", "not-the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            move |_| {
                flag.store(true, Ordering::SeqCst);
                async { Ok(tokens()) }
            },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(matches!(
            got,
            Err(ExchangeError::Callback(net_auth::CallbackError::Malformed))
        ));
        assert!(
            !called.load(Ordering::SeqCst),
            "the code was exchanged anyway"
        );
        assert!(store.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_decline_survives_as_a_decline() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();
        let url =
            url::Url::parse("http://127.0.0.1:1/callback?error=access_denied&state=the-state")
                .unwrap();

        let got = resolve_attempt(
            &url,
            "the-state",
            &cancel,
            Duration::from_secs(5),
            |_| async { Ok(tokens()) },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(matches!(
            got,
            Err(ExchangeError::Callback(net_auth::CallbackError::Denied))
        ));
    }

    #[tokio::test]
    async fn an_exchange_that_never_answers_is_bounded() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_millis(80),
            |_| async {
                // A backend that accepts the connection and never answers.
                std::future::pending::<()>().await;
                unreachable!()
            },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(matches!(got, Err(ExchangeError::Exchange(_))));
        assert!(
            store.lock().unwrap().is_empty(),
            "a timed-out attempt stored a credential"
        );
    }

    #[tokio::test]
    async fn a_cancel_during_the_exchange_discards_a_late_success() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let answered = Arc::new(AtomicBool::new(false));
        let flag = answered.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            token.cancel();
        });

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            move |_| async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                flag.store(true, Ordering::SeqCst);
                Ok(tokens())
            },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(matches!(got, Err(ExchangeError::Cancelled)));
        assert!(
            store.lock().unwrap().is_empty(),
            "a cancelled attempt stored a credential"
        );
        assert!(
            !answered.load(Ordering::SeqCst),
            "the exchange future outlived the cancelled attempt"
        );
    }

    #[tokio::test]
    async fn a_cancel_before_a_successful_answer_still_stores_nothing() {
        // 5.1d, stated deterministically: cancel FIRST, then let the exchange
        // answer successfully. Written as "cancel and answer at the same
        // moment" it would race tokio's branch selection and be flaky, which
        // is how such a test ends up deleted.
        //
        // Non-persistence inside `complete()` is necessary for this to pass
        // but NOT sufficient: it also needs the post-`select!` cancel
        // re-check. The test fails if either is missing, which is the point.
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();
        cancel.cancel();

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            |_| async { Ok(tokens()) },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(
            matches!(got, Err(ExchangeError::Cancelled)),
            "a cancelled attempt reported something other than cancelled"
        );
        assert!(
            store.lock().unwrap().is_empty(),
            "the keyring was written for an attempt the user cancelled"
        );
    }

    #[tokio::test]
    async fn a_failed_keyring_save_fails_the_sign_in() {
        // 5.1h: the Outcome is built from the keyring result too, so the tab
        // is sent to reason=error rather than to the signed-in page.
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            |_| async { Ok(tokens()) },
            recording_save(store.clone(), Some(0)),
        )
        .await;

        let err = match got {
            Err(ExchangeError::Exchange(e)) => e,
            Err(other) => panic!("expected a failed save, got {:?}", other),
            Ok(_) => panic!("a failed keyring save returned a session"),
        };
        assert_eq!(outcome_for(&err), Outcome::Error);
        assert!(store.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failure_of_the_second_write_leaves_nothing_stored() {
        // 5.1i: keyring_save runs twice, and refresh_session reads only the
        // first entry -- a half-written save would silently restore on the
        // next launch the session the user was just told had failed.
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            |_| async { Ok(tokens()) },
            recording_save(store.clone(), Some(1)),
        )
        .await;

        assert!(matches!(got, Err(ExchangeError::Exchange(_))));
        assert!(
            store.lock().unwrap().is_empty(),
            "the refresh token survived a failed second write"
        );
    }

    #[tokio::test]
    async fn an_unsupported_server_is_reported_distinctly() {
        let store = Arc::new(Mutex::new(Vec::new()));
        let cancel = CancellationToken::new();

        let got = resolve_attempt(
            &callback("abc", "the-state"),
            "the-state",
            &cancel,
            Duration::from_secs(5),
            |_| async { Err(net_auth::UNSUPPORTED_SERVER.to_string()) },
            recording_save(store.clone(), None),
        )
        .await;

        let err = match got {
            Err(ExchangeError::Exchange(e)) => e,
            Err(other) => panic!("expected the unsupported-server error, got {:?}", other),
            Ok(_) => panic!("an unsupported server returned a session"),
        };
        assert_eq!(outcome_for(&err), Outcome::Unsupported);
    }

    #[tokio::test]
    async fn the_first_attempts_callback_is_not_checked_against_the_second() {
        // 5.1c: a superseded attempt resolves as cancelled, not as tampering.
        // Showing the verification message for a sign-in nobody tampered with
        // is the failure this guards.
        let pending = PendingBrowserLogin::default();
        let first = CancellationToken::new();
        let first_id = pending.install(first.clone());
        let second = CancellationToken::new();
        pending.install(second.clone());

        let exchanges = Arc::new(AtomicUsize::new(0));
        let counter = exchanges.clone();
        let store = Arc::new(Mutex::new(Vec::new()));

        // The FIRST attempt's callback arrives, carrying the first attempt's
        // state. It is resolved against the first attempt's own token, which
        // was cancelled when the second superseded it.
        let got = resolve_attempt(
            &callback("abc", "first-state"),
            "first-state",
            &first,
            Duration::from_secs(5),
            move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                async { Ok(tokens()) }
            },
            recording_save(store.clone(), None),
        )
        .await;

        assert!(
            matches!(got, Err(ExchangeError::Cancelled)),
            "the superseded attempt did not resolve as cancelled"
        );
        assert!(store.lock().unwrap().is_empty());
        assert!(!second.is_cancelled(), "the live attempt was cancelled");
        pending.release(first_id);
        let _ = exchanges;
    }

    #[test]
    fn release_does_not_clear_a_newer_attempts_token() {
        let pending = PendingBrowserLogin::default();
        let first = CancellationToken::new();
        let second = CancellationToken::new();
        let first_id = pending.install(first.clone());
        pending.install(second.clone());

        // The superseded attempt finishes last and releases; the live
        // attempt's token has to survive that.
        pending.release(first_id);
        pending.cancel();
        assert!(second.is_cancelled(), "the live attempt's token was lost");
    }
}
