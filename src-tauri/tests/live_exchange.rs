//! Opt-in: the recorder's own exchange against a real API.
//!
//! OpenSpec change: browser-login, tasks 7.1/7.6. Every other test in this
//! crate stubs the server, which proves the recorder's handling of a response
//! shape but not that the deployed endpoint produces it. This one closes that
//! seam without needing a browser or a window.
//!
//! Ignored by default -- it needs a running API and a code minted against it,
//! and `cargo test` has to stay hermetic. To run it:
//!
//! ```text
//! # mint a code (see the change's archive note for the helper), then:
//! RECORDER_API_BASE=http://127.0.0.1:8000 \
//! RECORDER_CODE=<code> \
//! RECORDER_VERIFIER=<verifier> \
//!   cargo test --test live_exchange -- --ignored --nocapture
//! ```
//!
//! A code is single-use and lives 60 seconds, so it is minted immediately
//! before the run.

use sop_recorder_lib::network::auth;

#[tokio::test]
#[ignore = "needs a running API and a freshly minted code"]
async fn the_recorder_exchanges_a_real_code_for_a_real_session() {
    let api_base = std::env::var("RECORDER_API_BASE").expect("RECORDER_API_BASE");
    let code = std::env::var("RECORDER_CODE").expect("RECORDER_CODE");
    let verifier = std::env::var("RECORDER_VERIFIER").expect("RECORDER_VERIFIER");

    let pending = auth::PendingLogin {
        state: "unused-here".into(),
        verifier,
    };

    let tokens = match auth::complete(&pending, &code, &api_base).await {
        Ok(tokens) => tokens,
        Err(e) => panic!("the exchange failed: {}", e),
    };

    assert!(!tokens.access_token.is_empty(), "no access token");
    assert!(!tokens.refresh_token.is_empty(), "no refresh token");
    assert!(tokens.email.is_some(), "no email");
    println!(
        "exchanged: email={:?} access_token={} chars refresh_token={} chars",
        tokens.email,
        tokens.access_token.len(),
        tokens.refresh_token.len()
    );

    // Single-use: the same code must not work twice.
    let second = auth::complete(&pending, &code, &api_base).await;
    assert!(second.is_err(), "the code was exchangeable twice");
    println!("replay refused, as it should be");
}
