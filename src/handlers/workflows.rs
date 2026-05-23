//! `/api/v1/workflows/*` handlers.

use animus_control_protocol::types::{
    WorkflowCancelRequest, WorkflowGetRequest, WorkflowListRequest, WorkflowPauseRequest,
    WorkflowResumeRequest, WorkflowRunRequest,
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
    pub task_id: Option<String>,
    pub workflow_ref: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = WorkflowListRequest {
        status: q.status,
        task_id: q.task_id,
        workflow_ref: q.workflow_ref,
        limit: q.limit,
        offset: q.offset,
        ..Default::default()
    };
    wire_response(client.workflow_list(request).await)
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.workflow_get(WorkflowGetRequest { id }).await)
}

#[derive(Debug, Deserialize)]
pub struct RunBody {
    pub task_id: String,
    #[serde(default)]
    pub definition: Option<String>,
    #[serde(default)]
    pub params: Value,
}

pub async fn run(State(state): State<AppState>, Json(body): Json<RunBody>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let request = WorkflowRunRequest {
        task_id: body.task_id,
        definition: body.definition,
        params: body.params,
    };
    wire_response(client.workflow_run(request).await)
}

pub async fn pause(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.workflow_pause(WorkflowPauseRequest { id }).await)
}

#[derive(Debug, Default, Deserialize)]
pub struct ResumeBody {
    pub feedback: Option<String>,
}

pub async fn resume(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<ResumeBody>>,
) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let feedback = body.and_then(|b| b.0.feedback);
    wire_response(
        client
            .workflow_resume(WorkflowResumeRequest { id, feedback })
            .await,
    )
}

pub async fn cancel(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(client.workflow_cancel(WorkflowCancelRequest { id }).await)
}
