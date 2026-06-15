//! `/api/v1/subjects/*` handlers — typed subject CRUD over the control
//! client's `subject_*` verbs.
//!
//! This is distinct from the thin `/api/v1/subject/{plugin}/call` wrapper
//! (see [`crate::handlers::subject`]), which forwards an opaque method+params
//! envelope to a named subject backend. These handlers mirror the CLI's
//! `animus subject {list,get,create,update,next,status}` surface and route
//! through the daemon's `SubjectRouter` rather than targeting a plugin by
//! name.

use animus_control_protocol::types::{
    SubjectCreateRequest, SubjectGetRequest, SubjectListRequest, SubjectNextRequest,
    SubjectStatusRequest, SubjectUpdateRequest,
};
use animus_subject_protocol::{SubjectFilter, SubjectId, SubjectPatch, SubjectStatus};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::handlers::{connect, error_envelope, wire_response};
use crate::server::AppState;

fn invalid_input(msg: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(error_envelope("invalid_input", msg.into(), 2)),
    )
        .into_response()
}

fn parse_status(s: &str) -> Result<SubjectStatus, String> {
    serde_json::from_value(Value::String(s.to_string()))
        .map_err(|e| format!("invalid status `{s}`: {e}"))
}

/// Deserialize an `Option<Option<T>>` field that distinguishes an absent key
/// (`None`, leave untouched) from a present JSON `null` (`Some(None)`, clear).
/// Used in combination with `#[serde(default)]`: serde only invokes this when
/// the key is present, so a present `null` deserializes to `Some(None)`.
fn deserialize_optional_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

// ---- list -----------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    /// Restrict to a single subject kind (e.g. `task`).
    pub kind: Option<String>,
    /// Restrict to a single normalized status (e.g. `ready`).
    pub status: Option<String>,
    pub assignee: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

pub async fn list(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Response {
    // The daemon's `subject/list` requires a kind to route to a backend (it
    // returns InvalidRequest otherwise). Mirror the CLI, which requires
    // `--kind`, and reject up front with a 400 rather than a 502.
    let Some(kind) = q.kind.filter(|k| !k.is_empty()) else {
        return invalid_input("subject list requires a `kind` query parameter (e.g. ?kind=task)");
    };

    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };

    let status = match q.status.as_deref().map(parse_status).transpose() {
        Ok(s) => s,
        Err(err) => return invalid_input(err),
    };

    let filter = SubjectFilter {
        kind: vec![kind],
        status: status.into_iter().collect(),
        assignee: q.assignee.into_iter().collect(),
        cursor: q.cursor,
        limit: q.limit,
        ..SubjectFilter::default()
    };

    wire_response(client.subject_list(SubjectListRequest { filter }).await)
}

// ---- get ------------------------------------------------------------------

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(
        client
            .subject_get(SubjectGetRequest {
                id: SubjectId::new(id),
            })
            .await,
    )
}

// ---- create ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub custom: Map<String, Value>,
}

pub async fn create(State(state): State<AppState>, Json(body): Json<CreateBody>) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };

    if body.kind.is_empty() || body.title.is_empty() {
        return invalid_input("subject create requires non-empty `kind` and `title`");
    }

    let status = match body.status.as_deref().map(parse_status).transpose() {
        Ok(s) => s,
        Err(err) => return invalid_input(err),
    };

    let request = SubjectCreateRequest {
        kind: body.kind,
        title: body.title,
        body: body.body,
        status,
        priority: body.priority,
        labels: body.labels,
        assignee: body.assignee,
        custom: body.custom.into_iter().collect(),
    };
    wire_response(client.subject_create(request).await)
}

// ---- update ---------------------------------------------------------------

#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct UpdateBody {
    #[serde(default)]
    pub status: Option<String>,
    /// Set, change, or clear the assignee. A present JSON `null` clears it
    /// (`Some(None)`); an absent field leaves it untouched (`None`).
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub assignee: Option<Option<String>>,
    #[serde(default)]
    pub labels_add: Vec<String>,
    #[serde(default)]
    pub labels_remove: Vec<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub custom: Map<String, Value>,
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };

    let status = match body.status.as_deref().map(parse_status).transpose() {
        Ok(s) => s,
        Err(err) => return invalid_input(err),
    };

    let patch = SubjectPatch {
        status,
        assignee: body.assignee,
        labels_add: body.labels_add,
        labels_remove: body.labels_remove,
        comment: body.comment,
        custom: body.custom.into_iter().collect(),
    };
    let request = SubjectUpdateRequest {
        id: SubjectId::new(id),
        patch,
    };
    wire_response(client.subject_update(request).await)
}

// ---- next -----------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct NextQuery {
    pub kind: Option<String>,
}

pub async fn next(State(state): State<AppState>, Query(q): Query<NextQuery>) -> Response {
    // `subject/next` requires an explicit kind to choose a backend (the
    // daemon returns InvalidRequest otherwise), matching the CLI's required
    // `--kind`. Reject up front with a 400 rather than a 502.
    let Some(kind) = q.kind.filter(|k| !k.is_empty()) else {
        return invalid_input("subject next requires a `kind` query parameter (e.g. ?kind=task)");
    };
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    wire_response(
        client
            .subject_next(SubjectNextRequest { kind: Some(kind) })
            .await,
    )
}

// ---- status ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StatusBody {
    pub status: String,
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Response {
    let client = match connect(&state.settings.control_socket_path).await {
        Ok(c) => c,
        Err((code, body)) => return (code, body).into_response(),
    };
    let status = match parse_status(&body.status) {
        Ok(s) => s,
        Err(err) => return invalid_input(err),
    };
    let request = SubjectStatusRequest {
        id: SubjectId::new(id),
        status,
    };
    wire_response(client.subject_status(request).await)
}

#[cfg(test)]
mod tests {
    use super::UpdateBody;

    #[test]
    fn assignee_absent_leaves_untouched() {
        let body: UpdateBody = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(body.assignee, None);
    }

    #[test]
    fn assignee_null_clears() {
        let body: UpdateBody = serde_json::from_str(r#"{"assignee": null}"#).unwrap();
        assert_eq!(body.assignee, Some(None));
    }

    #[test]
    fn assignee_value_sets() {
        let body: UpdateBody = serde_json::from_str(r#"{"assignee": "alice"}"#).unwrap();
        assert_eq!(body.assignee, Some(Some("alice".to_string())));
    }
}
