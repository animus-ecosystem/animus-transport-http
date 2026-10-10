//! The transport only answers requests addressed to this machine: a DNS
//! rebinding page (foreign `Host`) or another site (foreign `Origin`) gets
//! 403 before any handler runs.

use std::path::PathBuf;

use animus_transport_http::config::HttpTransportSettings;
use animus_transport_http::server::build_router;
use animus_transport_http::HttpTransportBackend;
use animus_transport_protocol::{TransportBackend, TransportConfig};
use axum::body::Body;
use axum::http::{HeaderValue, Request, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

fn settings(allowed_hosts: &[&str]) -> HttpTransportSettings {
    HttpTransportSettings {
        bind_addr: "127.0.0.1:0".to_string(),
        control_socket_path: PathBuf::from("/tmp/animus-transport-http-nonexistent.sock"),
        project_root: PathBuf::from("/tmp"),
        allowed_hosts: allowed_hosts.iter().map(|h| h.to_string()).collect(),
    }
}

async fn status_with(
    allowed_hosts: &[&str],
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
) -> StatusCode {
    let mut req = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    build_router(settings(allowed_hosts))
        .oneshot(req.body(Body::from("{}")).unwrap())
        .await
        .unwrap()
        .status()
}

const HEALTH: &str = "/api/v1/daemon/health";
const RUN: &str = "/api/v1/workflows/run";

// With no daemon behind the test router, a request that gets past the guard
// is answered 503 by the handler.

#[tokio::test]
async fn local_requests_are_served() {
    for headers in [
        vec![("host", "127.0.0.1:8080")],
        vec![("host", "localhost:8080")],
        vec![("host", "[::1]:8080")],
        vec![
            ("host", "127.0.0.1:8080"),
            ("origin", "http://localhost:5173"),
        ],
    ] {
        let status = status_with(&[], "GET", HEALTH, &headers).await;
        assert_ne!(status, StatusCode::FORBIDDEN, "{headers:?}");
    }
}

#[tokio::test]
async fn rebound_host_is_refused() {
    for (method, path) in [("GET", HEALTH), ("POST", RUN)] {
        let status = status_with(&[], method, path, &[("host", "evil.example:8080")]).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
    }
}

#[tokio::test]
async fn cross_site_origin_is_refused() {
    let status = status_with(
        &[],
        "POST",
        RUN,
        &[
            ("host", "127.0.0.1:8080"),
            ("origin", "https://evil.example"),
            ("content-type", "text/plain"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // CORS preflight from another site.
    let status = status_with(
        &[],
        "OPTIONS",
        RUN,
        &[
            ("host", "127.0.0.1:8080"),
            ("origin", "https://evil.example"),
            ("access-control-request-method", "POST"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let status = status_with(
        &[],
        "GET",
        HEALTH,
        &[("host", "127.0.0.1:8080"), ("origin", "null")],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn request_without_host_is_refused() {
    for (method, path) in [("GET", HEALTH), ("POST", RUN)] {
        let status = status_with(&[], method, path, &[]).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
    }
}

#[tokio::test]
async fn unreadable_host_is_refused() {
    let req = Request::builder()
        .method("GET")
        .uri(HEALTH)
        .header("host", HeaderValue::from_bytes(b"127.0.0.1\xff").unwrap())
        .body(Body::empty())
        .unwrap();
    let status = build_router(settings(&[]))
        .oneshot(req)
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn local_uri_authority_without_host_is_served() {
    // HTTP/2 sends the host as the URI authority, not a Host header.
    let status = status_with(
        &[],
        "GET",
        "http://127.0.0.1:8080/api/v1/daemon/health",
        &[],
    )
    .await;
    assert_ne!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn allowed_hosts_extends_the_loopback_set() {
    let allowed = ["animus.railway.internal"];
    let status = status_with(
        &allowed,
        "GET",
        HEALTH,
        &[("host", "animus.railway.internal:8088")],
    )
    .await;
    assert_ne!(status, StatusCode::FORBIDDEN);

    let status = status_with(&allowed, "GET", HEALTH, &[("host", "other.internal:8088")]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Send raw HTTP/1.x to a started transport and return the status code.
async fn raw_status(addr: &str, request: &str) -> u16 {
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let response = String::from_utf8_lossy(&response);
    response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line in {response:?}"))
}

#[tokio::test]
async fn started_transport_applies_the_guard_on_the_socket() {
    let backend = HttpTransportBackend::default();
    let info = backend
        .start(TransportConfig {
            control_socket_path: PathBuf::from("/tmp/animus-transport-http-nonexistent.sock"),
            project_root: PathBuf::from("/tmp"),
            bind_addr: Some("127.0.0.1:0".to_string()),
            config: serde_json::json!({ "allowed_hosts": ["devbox.lan"] }),
        })
        .await
        .unwrap();
    let addr = info.bound_addr.as_str();

    let get =
        |host: &str| format!("GET {HEALTH} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    assert_eq!(raw_status(addr, &get("evil.example:8080")).await, 403);
    assert_eq!(raw_status(addr, &get("devbox.lan:8080")).await, 503);
    assert_eq!(raw_status(addr, &get(addr)).await, 503);
    // HTTP/1.0 allows leaving Host out.
    let no_host = format!("GET {HEALTH} HTTP/1.0\r\nConnection: close\r\n\r\n");
    assert_eq!(raw_status(addr, &no_host).await, 403);

    backend.shutdown().await.unwrap();
}

#[test]
fn manifest_forwards_allowed_hosts_but_not_the_bind_address() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_animus-transport-http"))
        .arg("--manifest")
        .output()
        .unwrap();
    assert!(output.status.success());
    let manifest: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let env = manifest["env_required"].as_array().unwrap();

    let allowed = env
        .iter()
        .find(|e| e["name"] == HttpTransportSettings::ALLOWED_HOSTS_ENV)
        .expect("ANIMUS_TRANSPORT_ALLOWED_HOSTS is declared");
    assert_eq!(allowed["required"], false);
    assert!(
        !env.iter()
            .any(|e| e["name"] == HttpTransportSettings::BIND_ENV),
        "declaring the bind env would expose deployments that set it; see main.rs"
    );
}
