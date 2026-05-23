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
