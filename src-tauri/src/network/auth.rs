use keyring::Entry;
use serde::Deserialize;

use crate::config;

const SERVICE_NAME: &str = "sop-sorcery";

pub fn api_url_for_target(upload_target: Option<&str>) -> &'static str {
    match upload_target {
        Some("Local") => config::API_URL_DEV,
        _ => config::API_URL_PROD,
    }
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

    if !res.status().is_success() {
        // Server rejected the token -- clear it
        let _ = keyring_clear("refresh-token");
        return Ok(None);
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
