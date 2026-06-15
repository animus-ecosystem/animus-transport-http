//! Smoke contract tests for the Axum router.
//!
//! These tests build the router without a live daemon and assert that the
//! routes exist (i.e. don't 404 at the routing layer). They are expected to
//! return 503 from handlers because there is no control socket — that's the
//! contract: transport plugins require a running daemon.

use std::path::PathBuf;

use animus_transport_http::config::HttpTransportSettings;
use animus_transport_http::server::build_router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn test_settings() -> HttpTransportSettings {
    HttpTransportSettings {
        bind_addr: "127.0.0.1:0".to_string(),
        control_socket_path: PathBuf::from("/tmp/animus-transport-http-nonexistent.sock"),
        project_root: PathBuf::from("/tmp"),
    }
}

async fn route_exists(method: &str, path: &str) -> StatusCode {
    let app = build_router(test_settings());
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    response.status()
}

#[tokio::test]
async fn daemon_status_route_exists() {
    let status = route_exists("GET", "/api/v1/daemon/status").await;
    // 503 (daemon unreachable) or 502 (wire error) are both acceptable —
    // the point is the route is mounted, not 404.
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn workflows_list_route_exists() {
    let status = route_exists("GET", "/api/v1/workflows").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn queue_stats_route_exists() {
    let status = route_exists("GET", "/api/v1/queue/stats").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn plugin_list_route_exists() {
    let status = route_exists("GET", "/api/v1/plugin/list").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn subject_call_route_exists() {
    let app = build_router(test_settings());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subject/markdown/call")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"method":"list","params":{}}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn subjects_list_route_exists() {
    let status = route_exists("GET", "/api/v1/subjects?kind=task").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn subjects_next_route_exists() {
    let status = route_exists("GET", "/api/v1/subjects/next?kind=task").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn subjects_list_without_kind_is_400() {
    // `kind` is required and validated before reaching the daemon, so this
    // returns a client-side 400 even with no control socket.
    let status = route_exists("GET", "/api/v1/subjects").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "got {status}");
}

#[tokio::test]
async fn subjects_next_without_kind_is_400() {
    let status = route_exists("GET", "/api/v1/subjects/next").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "got {status}");
}

#[tokio::test]
async fn subjects_create_route_exists() {
    let app = build_router(test_settings());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subjects")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"kind":"task","title":"hello"}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn subjects_get_route_exists() {
    let status = route_exists("GET", "/api/v1/subjects/task:T-1").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn subjects_status_route_exists() {
    let app = build_router(test_settings());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subjects/task:T-1/status")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"status":"ready"}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn workflows_execute_route_exists() {
    let app = build_router(test_settings());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/workflows/execute")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"definition":"build"}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn daemon_start_route_exists() {
    let status = route_exists("POST", "/api/v1/daemon/start").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn agent_routes_removed() {
    // Agent ops are CLI/in-process only — the kernel returns NotSupported
    // over control. The HTTP transport intentionally exposes no agent routes.
    let status = route_exists("POST", "/api/v1/agent/run").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "got {status}");
}

#[tokio::test]
async fn unknown_route_404s() {
    let status = route_exists("GET", "/api/v1/totally-not-a-route").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unreachable_daemon_returns_503_envelope() {
    let app = build_router(test_settings());
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/daemon/health")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "daemon_unreachable");
}
