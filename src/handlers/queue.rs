//! `/api/v1/queue/*` handlers.

use animus_control_protocol::types::{
    QueueDropRequest, QueueEnqueueRequest, QueueEntryStatus, QueueHoldRequest, QueueListRequest,
    QueueReleaseRequest, QueueReorderRequest,
};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::control_client::connect;
use crate::handlers::wire_response;
use crate::server::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let status = match q.status.as_deref().map(parse_status).transpose() {
        Ok(s) => s,
        Err(err) => return invalid_input(err),
    };
    let request = QueueListRequest {
        status,
        cursor: q.cursor,
        limit: q.limit,
    };
    wire_response(client.queue_list(request).await)
}

fn parse_status(s: &str) -> Result<QueueEntryStatus, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| format!("invalid status `{s}`: {e}"))
}

pub async fn stats(State(state): State<AppState>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.queue_stats().await)
}

pub async fn enqueue(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = match serde_json::from_value::<QueueEnqueueRequest>(body) {
        Ok(req) => req,
        Err(err) => return invalid_input(err.to_string()),
    };
    wire_response(client.queue_enqueue(request).await)
}

pub async fn reorder(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = match serde_json::from_value::<QueueReorderRequest>(body) {
        Ok(req) => req,
        Err(err) => return invalid_input(err.to_string()),
    };
    wire_response(client.queue_reorder(request).await)
}

pub async fn hold(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let reason = body.and_then(|b| {
        b.0.get("reason")
            .and_then(|v| v.as_str().map(str::to_string))
    });
    wire_response(client.queue_hold(QueueHoldRequest { id, reason }).await)
}

pub async fn release(
    State(state): State<AppState>,
    Path(id): Path<String>,
    _body: Option<Json<Value>>,
) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.queue_release(QueueReleaseRequest { id }).await)
}

pub async fn drop(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.queue_drop(QueueDropRequest { id }).await)
}

fn invalid_input(msg: String) -> Response {
    use crate::handlers::error_envelope;
    use axum::http::StatusCode;
    (
        StatusCode::BAD_REQUEST,
        Json(error_envelope("invalid_input", msg, 2)),
    )
        .into_response()
}
