//! Axum router assembly.
//!
//! Mounts the per-category handler modules under `/api/v1/*`. Every handler
//! routes through `ControlClient` against `settings.control_socket_path` —
//! no in-process service hub.

use std::time::Duration;

use axum::http::{header, Method};
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::HttpTransportSettings;
use crate::handlers;

#[derive(Clone)]
pub struct AppState {
    pub settings: HttpTransportSettings,
}

pub fn build_router(settings: HttpTransportSettings) -> Router {
    let state = AppState { settings };

    let api = Router::new()
        // daemon
        .route("/daemon/status", get(handlers::daemon::status))
        .route("/daemon/health", get(handlers::daemon::health))
        .route("/daemon/start", post(handlers::daemon::start))
        .route("/daemon/agents", get(handlers::daemon::agents))
        .route("/daemon/logs", get(handlers::daemon::logs))
        .route("/daemon/logs", delete(handlers::daemon::clear_logs))
        // workflows
        .route("/workflows", get(handlers::workflows::list))
        .route("/workflows/run", post(handlers::workflows::run))
        .route("/workflows/execute", post(handlers::workflows::execute))
        .route("/workflows/:id", get(handlers::workflows::get))
        .route("/workflows/:id/pause", post(handlers::workflows::pause))
        .route("/workflows/:id/resume", post(handlers::workflows::resume))
        .route("/workflows/:id/cancel", post(handlers::workflows::cancel))
        // queue
        .route("/queue", get(handlers::queue::list))
        .route("/queue/stats", get(handlers::queue::stats))
        .route("/queue/enqueue", post(handlers::queue::enqueue))
        .route("/queue/reorder", post(handlers::queue::reorder))
        .route("/queue/hold/:id", post(handlers::queue::hold))
        .route("/queue/release/:id", post(handlers::queue::release))
        .route("/queue/drop/:id", delete(handlers::queue::drop))
        // plugin
        .route("/plugin/list", get(handlers::plugin::list))
        .route("/plugin/info/:name", get(handlers::plugin::info))
        .route("/plugin/install", post(handlers::plugin::install))
        .route(
            "/plugin/uninstall/:name",
            delete(handlers::plugin::uninstall),
        )
        .route("/plugin/ping/:name", post(handlers::plugin::ping))
        .route("/plugin/call", post(handlers::plugin::call))
        .route("/plugin/search", get(handlers::plugin::search))
        .route("/plugin/browse", get(handlers::plugin::browse))
        .route("/plugin/update", post(handlers::plugin::update))
        // subject CRUD (typed routes through the daemon SubjectRouter)
        .route("/subjects", get(handlers::subject_ops::list))
        .route("/subjects", post(handlers::subject_ops::create))
        .route("/subjects/next", get(handlers::subject_ops::next))
        .route("/subjects/:id", get(handlers::subject_ops::get))
        .route("/subjects/:id", post(handlers::subject_ops::update))
        .route("/subjects/:id/status", post(handlers::subject_ops::status))
        // subject (thin wrapper over plugin/call for subject backends)
        .route("/subject/:plugin/call", post(handlers::subject::call));

    Router::new()
        .nest("/api/v1", api)
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin, _| {
                    origin.to_str().map(origin_allowed).unwrap_or(false)
                }))
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
                .max_age(Duration::from_secs(3600)),
        )
        .with_state(state)
}

fn origin_allowed(origin: &str) -> bool {
    ["http://localhost", "http://127.0.0.1"].iter().any(|base| {
        origin == *base
            || origin
                .strip_prefix(base)
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some_and(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
    })
}

#[cfg(test)]
mod tests {
    use super::origin_allowed;

    #[test]
    fn allows_localhost_origins() {
        assert!(origin_allowed("http://localhost"));
        assert!(origin_allowed("http://localhost:3000"));
        assert!(origin_allowed("http://127.0.0.1"));
        assert!(origin_allowed("http://127.0.0.1:8080"));
    }

    #[test]
    fn rejects_lookalike_origins() {
        assert!(!origin_allowed("http://localhost.evil.com"));
        assert!(!origin_allowed("http://127.0.0.1.evil.com"));
        assert!(!origin_allowed("http://localhost:3000.evil.com"));
        assert!(!origin_allowed("https://evil.com"));
        assert!(!origin_allowed("http://localhost:"));
    }
}
