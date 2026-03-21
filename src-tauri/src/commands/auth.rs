use serde::Serialize;
use std::sync::Mutex;
use tauri::State;
use tauri_plugin_store::StoreExt;

use crate::network::auth as net_auth;

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

fn get_api_base(app: &tauri::AppHandle) -> &'static str {
    let target = app
        .store("settings.json")
        .ok()
        .and_then(|store| {
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
pub async fn get_session_state(
    session: State<'_, SessionCache>,
) -> Result<SessionState, String> {
    let token = session.access_token.lock().unwrap().clone();
    let email = session.email.lock().unwrap().clone();
    Ok(SessionState {
        logged_in: token.is_some(),
        email,
    })
}
