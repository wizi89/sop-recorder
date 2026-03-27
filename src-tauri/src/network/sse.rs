use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SSEStatusPayload {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SSEResultPayload {
    pub enriched: Vec<serde_json::Value>,
    pub markdown: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SSEErrorPayload {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SSEPiiBlockedPayload {
    pub findings: serde_json::Value,
}

/// Consume SSE events from a reqwest Response, emitting Tauri events to the frontend.
/// Returns the final result payload if successful.
pub async fn consume_sse_stream(
    response: reqwest::Response,
    app: &tauri::AppHandle,
) -> Result<SSEResultPayload, String> {
    let url = response.url().clone();
    // reqwest-eventsource needs a RequestBuilder or we manually parse
    // Since we already have the response, we'll parse the body manually as SSE

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut result: Option<SSEResultPayload> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        let text = String::from_utf8_lossy(&chunk);
        log::debug!("SSE chunk ({} bytes): {:?}", chunk.len(), &text[..text.len().min(200)]);
        buffer.push_str(&text);

        // Normalise \r\n to \n so the delimiter search works with any server
        if buffer.contains("\r\n") {
            buffer = buffer.replace("\r\n", "\n");
        }

        // Parse SSE events from buffer
        while let Some(event_end) = buffer.find("\n\n") {
            let event_text = buffer[..event_end].to_string();
            buffer = buffer[event_end + 2..].to_string();

            let mut event_type = String::new();
            let mut data = String::new();

            for line in event_text.lines() {
                if let Some(t) = line.strip_prefix("event: ") {
                    event_type = t.trim().to_string();
                } else if let Some(d) = line.strip_prefix("data: ") {
                    data = d.trim().to_string();
                }
            }

            log::debug!("SSE event: type={:?}, data_len={}", event_type, data.len());

            match event_type.as_str() {
                "status" => {
                    if let Ok(payload) =
                        serde_json::from_str::<SSEStatusPayload>(&data)
                    {
                        let _ = app.emit("sse:status", &payload);
                    } else {
                        // Plain text status
                        let _ = app.emit(
                            "sse:status",
                            SSEStatusPayload {
                                message: data.clone(),
                            },
                        );
                    }
                }
                "result" => {
                    match serde_json::from_str::<SSEResultPayload>(&data) {
                        Ok(payload) => {
                            let _ = app.emit("sse:result", &payload);
                            result = Some(payload);
                        }
                        Err(e) => {
                            log::error!("Failed to parse SSE result: {}", e);
                        }
                    }
                }
                "pii_blocked" => {
                    if let Ok(payload) =
                        serde_json::from_str::<SSEPiiBlockedPayload>(&data)
                    {
                        let _ = app.emit("sse:pii_blocked", &payload);
                        return Err("PII blocked".into());
                    }
                }
                "error" => {
                    log::error!("SSE error event received: {}", data);
                    let msg = serde_json::from_str::<SSEErrorPayload>(&data)
                        .map(|p| p.message)
                        .unwrap_or(data);
                    let _ = app.emit(
                        "sse:error",
                        SSEErrorPayload {
                            message: msg.clone(),
                        },
                    );
                    return Err(msg);
                }
                _ => {
                    // Unknown event type, ignore
                }
            }
        }
    }

    log::info!("SSE stream ended. buffer_remaining={} has_result={}", buffer.len(), result.is_some());
    if !buffer.trim().is_empty() {
        log::debug!("SSE leftover buffer: {:?}", &buffer[..buffer.len().min(500)]);
    }

    let _ = url; // suppress unused warning

    result.ok_or_else(|| {
        "Server closed connection without sending a result".to_string()
    })
}
