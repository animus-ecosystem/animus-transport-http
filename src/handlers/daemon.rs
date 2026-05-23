//! `/api/v1/daemon/*` handlers.

use animus_control_protocol::types::DaemonLogsRequest;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::control_client::connect;
use crate::handlers::{error_envelope, wire_response};
use crate::server::AppState;

pub async fn status(State(state): State<AppState>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.daemon_status().await)
}

pub async fn health(State(state): State<AppState>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.daemon_health().await)
}

pub async fn agents(State(state): State<AppState>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.daemon_agents().await)
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub limit: Option<usize>,
}

pub async fn logs(State(state): State<AppState>, Query(q): Query<LogsQuery>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = DaemonLogsRequest::default();
    let limit = q.limit.unwrap_or(200);
    wire_response(client.daemon_logs(request, limit).await)
}

pub async fn clear_logs(State(_state): State<AppState>) -> Response {
    // Clearing daemon logs is a side-effecting operation that lives on the
    // CLI/web-api today. Until a dedicated wire verb exists, return 501.
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(error_envelope(
            "not_implemented",
            "daemon/logs DELETE not yet wired over control RPC",
            21,
        )),
    )
        .into_response()
}
