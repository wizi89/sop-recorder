use super::*;
use crate::config;

#[test]
fn only_a_rejection_of_the_token_signs_the_user_out() {
    for status in [400, 401, 403] {
        assert!(token_rejected(status), "{status}");
    }
    for status in [404, 408, 429, 500, 502, 503, 504] {
        assert!(!token_rejected(status), "{status} must keep the session");
    }
}

#[test]
fn api_url_defaults_to_prod() {
    assert_eq!(api_url_default(None), config::API_URL_PROD);
}

#[test]
fn api_url_local_target_returns_dev() {
    assert_eq!(api_url_default(Some("Local")), config::API_URL_DEV);
}

#[test]
fn api_url_staging_target_returns_staging() {
    assert_eq!(api_url_default(Some("Staging")), config::API_URL_STAGING);
}

#[test]
fn api_url_production_target_returns_prod() {
    assert_eq!(api_url_default(Some("Production")), config::API_URL_PROD);
}

#[test]
fn api_url_unknown_target_returns_prod() {
    assert_eq!(api_url_default(Some("anything")), config::API_URL_PROD);
}

// -- Browser sign-in ----------------------------------------------------

#[test]
fn begin_mints_distinct_values() {
    let (a, _) = begin();
    let (b, _) = begin();
    assert_ne!(a.state, b.state);
    assert_ne!(a.verifier, b.verifier);
    assert_ne!(a.state, a.verifier);
}

#[test]
fn verifier_is_64_hex_characters() {
    let (p, _) = begin();
    assert_eq!(p.verifier.len(), 64);
    assert!(p.verifier.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(p.verifier.chars().all(|c| !c.is_ascii_uppercase()));
}

#[test]
fn challenge_is_unpadded_base64url() {
    let (_, challenge) = begin();
    assert_eq!(challenge.len(), 43);
    assert!(!challenge.contains('='));
    assert!(!challenge.contains('+'));
    assert!(!challenge.contains('/'));
}

#[test]
fn challenge_is_pinned_against_a_known_verifier() {
    // base64url(sha256("desktop-login-e2e-verifier")), unpadded. Pins the
    // hash and the encoding together: a refactor that changed either would
    // still produce plausible-looking output.
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(b"desktop-login-e2e-verifier"));
    assert_eq!(challenge, "BBEFDbvf-kCGh06wdyew7yb8THggGdRTiklR2NvJTSs");
}

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn parses_a_loopback_callback() {
    let got = parse_callback(&url("http://127.0.0.1:53124/callback?code=abc&state=xyz"));
    assert_eq!(got, Ok(("abc".into(), "xyz".into())));
}

#[test]
fn parses_a_custom_scheme_callback_identically() {
    let loopback = parse_callback(&url("http://127.0.0.1:53124/callback?code=abc&state=xyz"));
    let scheme = parse_callback(&url("cogniclone://auth/callback?code=abc&state=xyz"));
    assert_eq!(loopback, scheme);
}

#[test]
fn percent_encoded_state_round_trips() {
    let got = parse_callback(&url(
        "http://127.0.0.1:1/callback?code=a%2Bb&state=s%3D%26%3F",
    ));
    assert_eq!(got, Ok(("a+b".into(), "s=&?".into())));
}

#[test]
fn access_denied_is_a_decline() {
    assert_eq!(
        parse_callback(&url(
            "http://127.0.0.1:1/callback?error=access_denied&state=x"
        )),
        Err(CallbackError::Denied)
    );
}

#[test]
fn server_error_is_its_own_variant() {
    assert_eq!(
        parse_callback(&url(
            "http://127.0.0.1:1/callback?error=server_error&state=x"
        )),
        Err(CallbackError::ServerError)
    );
}

#[test]
fn an_unknown_error_value_is_malformed() {
    assert_eq!(
        parse_callback(&url("http://127.0.0.1:1/callback?error=whatever&state=x")),
        Err(CallbackError::Malformed)
    );
}

#[test]
fn a_missing_code_or_state_is_malformed() {
    for u in [
        "http://127.0.0.1:1/callback?state=xyz",
        "http://127.0.0.1:1/callback?code=abc",
        "http://127.0.0.1:1/callback",
        "http://127.0.0.1:1/callback?code=&state=xyz",
        "http://127.0.0.1:1/callback?code=abc&state=",
    ] {
        assert_eq!(
            parse_callback(&url(u)),
            Err(CallbackError::Malformed),
            "{}",
            u
        );
    }
}

// -- complete() against real HTTP responses (task 3.1) ------------------

/// `unwrap_err` would require `Debug` on `Tokens`, and a struct holding an
/// access token should not be printable by accident.
fn err_of(result: Result<Tokens, String>) -> String {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected an error, got a session"),
    }
}

/// A one-shot HTTP server that answers with exactly what it is given.
///
/// `None` means: accept the connection and never answer, which is the case
/// `sign_in`'s timeout-free client would hang on forever.
async fn stub_server(response: Option<&'static str>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf).await;
            match response {
                Some(body) => {
                    let _ = stream.write_all(body.as_bytes()).await;
                    let _ = stream.flush().await;
                }
                None => {
                    // Held open, silent. Dropped only when the test ends.
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }
    });

    format!("http://127.0.0.1:{}", port)
}

fn http(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        content_type,
        body.len(),
        body
    )
}

#[tokio::test]
async fn complete_returns_the_tokens_on_the_happy_path() {
    let base = stub_server(Some(Box::leak(
        http(
            "200 OK",
            "application/json",
            r#"{"access_token":"at-1","refresh_token":"rt-1","email":"user@example.test"}"#,
        )
        .into_boxed_str(),
    )))
    .await;
    let (p, _) = begin();

    let tokens = complete(&p, "the-code", &base).await.unwrap();
    assert_eq!(tokens.access_token, "at-1");
    assert_eq!(tokens.refresh_token, "rt-1");
    assert_eq!(tokens.email.as_deref(), Some("user@example.test"));
}

#[tokio::test]
async fn complete_persists_nothing() {
    // The property the cancel path depends on: the command writes the
    // keyring after the select!, never complete() itself.
    let source = include_str!("../auth.rs").replace("\r\n", "\n");
    let body = source
        .split("async fn complete_within(")
        .nth(1)
        .expect("complete_within not found")
        .split("\n/// Persist what the exchange returned")
        .next()
        .unwrap();
    assert!(
        !body.contains("keyring_save"),
        "complete() writes the keyring: a cancel that wins the race would \
         leave a stored refresh token behind a UI that said cancelled"
    );
}

#[tokio::test]
async fn a_404_means_the_server_does_not_support_browser_sign_in() {
    // What an API deployed before this change answers. `sign_in` would
    // have surfaced it as an ordinary failed login.
    let base = stub_server(Some(Box::leak(
        http(
            "404 Not Found",
            "application/json",
            r#"{"detail":"Not Found"}"#,
        )
        .into_boxed_str(),
    )))
    .await;
    let (p, _) = begin();

    let err = err_of(complete(&p, "the-code", &base).await);
    assert_eq!(err, UNSUPPORTED_SERVER);
}

#[tokio::test]
async fn a_405_means_the_same() {
    let base = stub_server(Some(Box::leak(
        http("405 Method Not Allowed", "application/json", "{}").into_boxed_str(),
    )))
    .await;
    let (p, _) = begin();

    assert_eq!(
        err_of(complete(&p, "the-code", &base).await),
        UNSUPPORTED_SERVER
    );
}

#[tokio::test]
async fn a_body_that_is_not_json_means_the_same() {
    // An HTML error page from a proxy, which `sign_in` would have reported
    // as "Anmeldung fehlgeschlagen".
    let base = stub_server(Some(Box::leak(
        http(
            "502 Bad Gateway",
            "text/html",
            "<html><body><h1>502 Bad Gateway</h1></body></html>",
        )
        .into_boxed_str(),
    )))
    .await;
    let (p, _) = begin();

    assert_eq!(
        err_of(complete(&p, "the-code", &base).await),
        UNSUPPORTED_SERVER
    );
}

#[tokio::test]
async fn the_probe_reports_an_api_without_the_endpoint() {
    let base = stub_server(Some(Box::leak(
        http(
            "404 Not Found",
            "application/json",
            r#"{"detail":"Not Found"}"#,
        )
        .into_boxed_str(),
    )))
    .await;

    assert_eq!(probe_exchange(&base).await.unwrap_err(), UNSUPPORTED_SERVER);
}

#[tokio::test]
async fn the_probe_passes_an_api_that_refuses_the_empty_body() {
    // What staging answers: the endpoint is there and wants a code.
    let base = stub_server(Some(Box::leak(
        http(
            "400 Bad Request",
            "application/json",
            r#"{"error":"invalid_code","message":"The sign-in code is invalid or has expired."}"#,
        )
        .into_boxed_str(),
    )))
    .await;

    assert!(probe_exchange(&base).await.is_ok());
}

#[tokio::test]
async fn the_probe_does_not_block_sign_in_when_it_cannot_tell() {
    // Silence and an unreachable host both say nothing about the
    // endpoint, so the attempt goes ahead exactly as it did before.
    let silent = stub_server(None).await;
    assert!(
        probe_exchange_within(&silent, std::time::Duration::from_millis(200))
            .await
            .is_ok()
    );
    assert!(probe_exchange("http://127.0.0.1:1").await.is_ok());
}

#[tokio::test]
async fn a_refused_exchange_reports_the_servers_message() {
    let base = stub_server(Some(Box::leak(
        http(
            "400 Bad Request",
            "application/json",
            r#"{"message":"The sign-in code is invalid or has expired."}"#,
        )
        .into_boxed_str(),
    )))
    .await;
    let (p, _) = begin();

    let err = err_of(complete(&p, "the-code", &base).await);
    assert!(err.contains("invalid or has expired"), "{}", err);
    assert_ne!(err, UNSUPPORTED_SERVER);
}

#[tokio::test]
async fn a_server_that_never_answers_times_out_rather_than_hanging() {
    let base = stub_server(None).await;
    let (p, _) = begin();

    let started = tokio::time::Instant::now();
    let err =
        err_of(complete_within(&p, "the-code", &base, std::time::Duration::from_millis(150)).await);

    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "it hung"
    );
    assert!(err.contains("zu lange"), "{}", err);
}

#[tokio::test]
async fn the_exchange_sends_the_code_and_the_verifier_and_nothing_else() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let captured = seen.clone();

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await.unwrap();
        *captured.lock().unwrap_or_else(|e| e.into_inner()) =
            String::from_utf8_lossy(&buf[..n]).to_string();
        let body = r#"{"access_token":"at","refresh_token":"rt","email":"u@e.test"}"#;
        let _ = stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await;
        let _ = stream.flush().await;
    });

    let (p, challenge) = begin();
    complete(&p, "the-code", &format!("http://127.0.0.1:{}", port))
        .await
        .unwrap();

    let request = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert!(
        request.contains("POST /auth/desktop/exchange"),
        "{}",
        request
    );
    assert!(request.contains("\"code\":\"the-code\""), "{}", request);
    assert!(request.contains(&format!("\"code_verifier\":\"{}\"", p.verifier)));
    // The challenge stays on this side of the wire: the server already has
    // it, bound to the code.
    assert!(!request.contains(&challenge), "the challenge was sent too");
    assert!(!request.contains(&p.state), "the state was sent too");
}

#[test]
fn constant_time_eq_agrees_with_equality() {
    assert!(constant_time_eq("abc", "abc"));
    assert!(!constant_time_eq("abc", "abd"));
    assert!(!constant_time_eq("abc", "abcd"));
    assert!(!constant_time_eq("", "a"));
    assert!(constant_time_eq("", ""));
}
