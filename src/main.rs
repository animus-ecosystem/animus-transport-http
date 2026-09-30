//! `animus-transport-http` binary entrypoint.
//!
//! Wires the [`HttpTransportBackend`] into [`transport_backend_main`] so the
//! Animus plugin host can drive its lifecycle over stdio.

use animus_plugin_protocol::PluginInfo;
use animus_plugin_runtime::transport_backend_main;
use animus_transport_protocol::PLUGIN_KIND_TRANSPORT_BACKEND;

use animus_transport_http::HttpTransportBackend;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    emit_manifest_if_requested();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let backend = HttpTransportBackend::default();
    let info = PluginInfo {
        name: env!("CARGO_PKG_NAME").into(),
        version: env!("CARGO_PKG_VERSION").into(),
        plugin_kind: PLUGIN_KIND_TRANSPORT_BACKEND.into(),
        description: Some(env!("CARGO_PKG_DESCRIPTION").into()),
    };

    transport_backend_main(info, backend).await
}

fn emit_manifest_if_requested() {
    if !std::env::args()
        .skip(1)
        .any(|arg| arg == "--manifest" || arg == "-m")
    {
        return;
    }

    let manifest = serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "plugin_kind": "transport_backend",
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "protocol_version": animus_plugin_protocol::PROTOCOL_VERSION,
        "capabilities": [
            "transport/start",
            "transport/shutdown",
            "transport/schema",
            "health/check"
        ],
        // The plugin host starts plugins with a cleared environment and only
        // forwards variables declared here. ANIMUS_TRANSPORT_ALLOWED_HOSTS is
        // optional: unset, only localhost and loopback IPs are accepted.
        //
        // ANIMUS_TRANSPORT_HTTP_BIND is deliberately NOT declared. Existing
        // container deployments set it to 0.0.0.0:8080 while the host has
        // been dropping it, so their unauthenticated API stays on loopback
        // behind their own authenticating forwarder. Declaring it would
        // expose the API on their network without that check.
        "env_required": [
            {
                "name": "ANIMUS_TRANSPORT_ALLOWED_HOSTS",
                "description": "Comma-separated host names to accept besides localhost and loopback IPs",
                "sensitive": false,
                "required": false
            }
        ]
    });
    println!(
        "{}",
        serde_json::to_string(&manifest).expect("serialize manifest")
    );
    std::process::exit(0);
}
