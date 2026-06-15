//! `/api/v1/workflows/*` handlers.

use std::collections::BTreeMap;

use animus_control_protocol::types::{
    WorkflowCancelRequest, WorkflowExecuteRequest, WorkflowGetRequest, WorkflowListRequest,
    WorkflowPauseRequest, WorkflowResumeRequest, WorkflowRunRequest, WorkflowStatus,
};
use animus_subject_protocol::SubjectId;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::handlers::{connect, wire_response};
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
    let request = WorkflowListRequest {
        status,
        cursor: q.cursor,
        limit: q.limit,
    };
    wire_response(client.workflow_list(request).await)
}

fn parse_status(s: &str) -> Result<WorkflowStatus, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|e| format!("invalid status `{s}`: {e}"))
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
    let params = match value_to_param_map(body.params) {
        Ok(p) => p,
        Err(err) => return invalid_input(err),
    };
    let request = WorkflowRunRequest {
        task_id: body.task_id,
        definition: body.definition,
        params,
    };
    wire_response(client.workflow_run(request).await)
}

#[derive(Debug, Deserialize)]
pub struct ExecuteBody {
    /// Workflow definition name to run.
    pub definition: String,
    /// Optional subject id to associate the run with.
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub params: Value,
}

pub async fn execute(State(state): State<AppState>, Json(body): Json<ExecuteBody>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    if body.definition.is_empty() {
        return invalid_input("workflow execute requires a non-empty `definition`".to_string());
    }
    let params = match value_to_param_map(body.params) {
        Ok(p) => p,
        Err(err) => return invalid_input(err),
    };
    let request = WorkflowExecuteRequest {
        definition: body.definition,
        params,
        subject_id: body.subject_id.map(SubjectId::new),
    };
    wire_response(client.workflow_execute(request).await)
}

fn value_to_param_map(value: Value) -> Result<BTreeMap<String, Value>, String> {
    match value {
        Value::Null => Ok(BTreeMap::new()),
        Value::Object(map) => Ok(map.into_iter().collect()),
        other => Err(format!(
            "`params` must be a JSON object, got {}",
            match other {
                Value::Array(_) => "array",
                Value::String(_) => "string",
                Value::Number(_) => "number",
                Value::Bool(_) => "boolean",
                Value::Null | Value::Object(_) => unreachable!(),
            }
        )),
    }
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
    wire_response(
        client
            .workflow_cancel(WorkflowCancelRequest { id, reason: None })
            .await,
    )
}
