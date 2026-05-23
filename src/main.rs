//! `animus-transport-http` binary entrypoint.
//!
//! Wires the [`HttpTransportBackend`] into [`transport_backend_main`] so the
//! Animus plugin host can drive its lifecycle over stdio.

use animus_plugin_protocol::{PluginInfo, PLUGIN_KIND_TRANSPORT_BACKEND};
use animus_plugin_runtime::transport_backend_main;

use animus_transport_http::HttpTransportBackend;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
