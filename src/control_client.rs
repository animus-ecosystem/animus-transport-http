//! Thin connection helper around the animus daemon control client.
//!
//! Transport plugins assume the daemon is running. This module wraps the
//! connect step into a single function so every handler can fail loudly with
//! a 503 if the control socket is missing rather than silently fall back to
//! an in-process path (transport plugins have no such path).

use std::path::Path;

use animus_control_protocol::ControlClient;
use axum::http::StatusCode;
use axum::Json;

use crate::handlers::error_envelope;

/// Open a control client against the given socket path. Returns an HTTP 503
/// response on failure so handlers can `?` straight back out.
pub async fn connect(
    socket_path: &Path,
) -> Result<ControlClient, (StatusCode, Json<serde_json::Value>)> {
    match ControlClient::connect(socket_path).await {
        Ok(client) => Ok(client),
        Err(err) => {
            tracing::warn!(
                error = %err,
                socket = %socket_path.display(),
                "control socket unreachable"
            );
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_envelope(
                    "daemon_unreachable",
                    format!(
                        "could not reach animus daemon at {}: {err}",
                        socket_path.display()
                    ),
                    20,
                )),
            ))
        }
    }
}
