//! REST handlers. Every handler resolves the daemon control socket from
//! `AppState`, opens a `ControlClient`, dispatches the matching wire verb,
//! and serializes the response as a `CliEnvelope`-shaped JSON body.
//!
//! There is no in-process fallback — transport plugins assume the daemon is
//! running. If the socket is unreachable the handler returns 503.

pub mod agent;
pub mod daemon;
pub mod plugin;
pub mod queue;
pub mod subject;
pub mod workflows;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

/// Build the standard success envelope (`{"ok": true, "data": ...}`).
pub fn ok_envelope<T: Serialize>(data: T) -> Value {
    json!({
        "ok": true,
        "data": data,
    })
}

/// Build the standard error envelope.
pub fn error_envelope(code: impl Into<String>, message: impl Into<String>, exit_code: i32) -> Value {
    json!({
        "ok": false,
        "error": {
            "code": code.into(),
            "message": message.into(),
            "exit_code": exit_code,
        },
    })
}

/// Convert any `Result<T, anyhow::Error>` into a JSON response. Uses 200 on
/// success and 502 with the daemon's error message on failure.
pub fn wire_response<T: Serialize>(result: anyhow::Result<T>) -> Response {
    match result {
        Ok(value) => (StatusCode::OK, Json(ok_envelope(value))).into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "wire call failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(error_envelope("wire_error", err.to_string(), 1)),
            )
                .into_response()
        }
    }
}
