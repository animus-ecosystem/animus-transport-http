//! `/api/v1/subject/{plugin}/call` — thin wrapper over `plugin/call` so HTTP
//! clients can target a subject backend by name without constructing the full
//! `PluginCallRequest` envelope themselves. Subject-specific routing
//! ultimately lives in the daemon.

use animus_control_protocol::types::PluginCallRequest;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::control_client::connect;
use crate::handlers::{error_envelope, wire_response};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct SubjectCallBody {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub async fn call(
    State(state): State<AppState>,
    Path(plugin): Path<String>,
    Json(body): Json<SubjectCallBody>,
) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };

    if body.method.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(error_envelope(
                "invalid_input",
                "subject call requires a `method` field",
                2,
            )),
        )
            .into_response();
    }

    let request = PluginCallRequest {
        name: plugin,
        method: body.method,
        params: body.params,
        ..Default::default()
    };
    wire_response(client.plugin_call(request).await)
}
