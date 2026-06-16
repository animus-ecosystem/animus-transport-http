//! `TransportConfig` parsing helpers.
//!
//! The animus daemon hands the plugin a `TransportConfig` at start time. The
//! shape is owned by `animus-transport-protocol` — this module just provides
//! small helpers to extract the values this plugin needs (bind address,
//! control socket path, project root).

use std::path::PathBuf;

use animus_transport_protocol::TransportConfig;

/// Plugin-local view of the relevant config fields.
#[derive(Debug, Clone)]
pub struct HttpTransportSettings {
    pub bind_addr: String,
    pub control_socket_path: PathBuf,
    pub project_root: PathBuf,
}

impl HttpTransportSettings {
    pub const DEFAULT_BIND_ADDR: &'static str = "127.0.0.1:8080";

    /// Env override for the bind address. `animus web serve` omits `bind_addr`
    /// from the transport/start config (the plugin falls back to its default
    /// 127.0.0.1:8080), which is unreachable when the transport runs in a
    /// container or behind a private network. This env var lets a deployment
    /// bind 0.0.0.0 without a kernel/CLI change.
    pub const BIND_ENV: &'static str = "ANIMUS_TRANSPORT_HTTP_BIND";

    /// Extract the HTTP-relevant fields from the supplied transport config.
    /// Precedence: explicit `config.bind_addr` > `$ANIMUS_TRANSPORT_HTTP_BIND` > default.
    pub fn from_config(config: &TransportConfig) -> Self {
        Self {
            bind_addr: config
                .bind_addr
                .clone()
                .or_else(|| {
                    std::env::var(Self::BIND_ENV)
                        .ok()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                })
                .unwrap_or_else(|| Self::DEFAULT_BIND_ADDR.to_string()),
            control_socket_path: config.control_socket_path.clone(),
            project_root: config.project_root.clone(),
        }
    }
}
