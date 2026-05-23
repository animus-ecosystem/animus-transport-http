//! `/api/v1/plugin/*` handlers.

use animus_control_protocol::types::{
    PluginBrowseRequest, PluginCallRequest, PluginInfoRequest, PluginInstallRequest,
    PluginListRequest, PluginPingRequest, PluginSearchRequest, PluginUninstallRequest,
    PluginUpdateRequest,
};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::control_client::connect;
use crate::handlers::{error_envelope, wire_response};
use crate::server::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    pub kind: Option<String>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = PluginListRequest {
        kind: q.kind,
        include_warnings: false,
    };
    wire_response(client.plugin_list(request).await)
}

pub async fn info(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.plugin_info(PluginInfoRequest { name }).await)
}

pub async fn install(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = match serde_json::from_value::<PluginInstallRequest>(body) {
        Ok(req) => req,
        Err(err) => return invalid_input(err.to_string()),
    };
    wire_response(client.plugin_install(request).await)
}

pub async fn uninstall(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(
        client
            .plugin_uninstall(PluginUninstallRequest { name })
            .await,
    )
}

pub async fn ping(State(state): State<AppState>, Path(name): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.plugin_ping(PluginPingRequest { name }).await)
}

pub async fn call(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = match serde_json::from_value::<PluginCallRequest>(body) {
        Ok(req) => req,
        Err(err) => return invalid_input(err.to_string()),
    };
    wire_response(client.plugin_call(request).await)
}

#[derive(Debug, Default, Deserialize)]
pub struct SearchQuery {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub tag: Option<String>,
}

pub async fn search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = PluginSearchRequest {
        query: q.query.unwrap_or_default(),
        kind: q.kind,
        tag: q.tag,
    };
    wire_response(client.plugin_search(request).await)
}

#[derive(Debug, Default, Deserialize)]
pub struct BrowseQuery {
    pub kind: Option<String>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub available: bool,
}

pub async fn browse(State(state): State<AppState>, Query(q): Query<BrowseQuery>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = PluginBrowseRequest {
        kind: q.kind,
        installed: q.installed,
        available: q.available,
    };
    wire_response(client.plugin_browse(request).await)
}

pub async fn update(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = match serde_json::from_value::<PluginUpdateRequest>(body) {
        Ok(req) => req,
        Err(err) => return invalid_input(err.to_string()),
    };
    wire_response(client.plugin_update(request).await)
}

fn invalid_input(msg: String) -> Response {
    use axum::http::StatusCode;
    (
        StatusCode::BAD_REQUEST,
        Json(error_envelope("invalid_input", msg, 2)),
    )
        .into_response()
}
