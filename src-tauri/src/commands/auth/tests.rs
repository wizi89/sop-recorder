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
                store.lock().unwrap_or_else(|e| e.into_inner()).clear(); // sign_out()
                return Err("credential store failed".into());
            }
            store
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((*key).to_string());
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
    assert_eq!(
        *store.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["refresh-token", "email"]
    );
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
    assert!(store.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
}

#[tokio::test]
async fn a_decline_survives_as_a_decline() {
    let store = Arc::new(Mutex::new(Vec::new()));
    let cancel = CancellationToken::new();
    let url =
        url::Url::parse("http://127.0.0.1:1/callback?error=access_denied&state=the-state").unwrap();

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
        store.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
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
        store.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
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
        store.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
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
    assert!(store.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
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
        store.lock().unwrap_or_else(|e| e.into_inner()).is_empty(),
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
    assert!(store.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
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
