use tauri::State;

use crate::commands::auth::{get_api_base, SessionCache};
use crate::network::{auth as net_auth, quota as net_quota};

/// Fetch the authenticated user's generation quota from the server.
///
/// Refreshes the session first so the Bearer token is always fresh; falls
/// back to the cached token if refresh fails (for example no network).
#[tauri::command]
pub async fn get_quota(
    app: tauri::AppHandle,
    session: State<'_, SessionCache>,
) -> Result<net_quota::Quota, String> {
    let api_base = get_api_base(&app);

    // Try to refresh the session so the token we use is guaranteed fresh.
    // If refresh fails (no refresh token stored, network error), fall back
    // to whatever is in the session cache.
    let access_token = match net_auth::refresh_session(api_base).await {
        Ok(Some(auth)) => {
            let token = auth.access_token.clone();
            *session.access_token.lock().unwrap() = Some(auth.access_token);
            if let Some(email) = auth.email {
                *session.email.lock().unwrap() = Some(email);
            }
            token
        }
        _ => session
            .access_token
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| "Nicht angemeldet".to_string())?,
    };

    net_quota::fetch_quota(&access_token, Some(api_base)).await
}
