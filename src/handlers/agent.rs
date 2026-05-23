//! `/api/v1/agent/*` handlers.

use animus_control_protocol::types::{AgentCancelRequest, AgentRunRequest, AgentStatusRequest};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;

use crate::control_client::connect;
use crate::handlers::{error_envelope, wire_response};
use crate::server::AppState;

pub async fn run(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = match serde_json::from_value::<AgentRunRequest>(body) {
        Ok(req) => req,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(error_envelope("invalid_input", err.to_string(), 2)),
            )
                .into_response();
        }
    };
    wire_response(client.agent_run(request).await)
}

pub async fn status(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.agent_status(AgentStatusRequest { id }).await)
}

pub async fn cancel(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.agent_cancel(AgentCancelRequest { id }).await)
}
