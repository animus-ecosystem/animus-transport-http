//! `TransportConfig` parsing helpers.
//!
//! The animus daemon hands the plugin a `TransportConfig` at start time. The
//! shape is owned by `animus-transport-protocol` — this module just provides
//! small helpers to extract the values this plugin needs (bind address,
//! control socket path, project root, extra allowed host names).

use std::path::PathBuf;

use animus_transport_protocol::TransportConfig;

/// Plugin-local view of the relevant config fields.
#[derive(Debug, Clone)]
pub struct HttpTransportSettings {
    pub bind_addr: String,
    pub control_socket_path: PathBuf,
    pub project_root: PathBuf,
    /// Host names accepted besides localhost and loopback IPs. See
    /// [`crate::loopback`].
    pub allowed_hosts: Vec<String>,
}

impl HttpTransportSettings {
    pub const DEFAULT_BIND_ADDR: &'static str = "127.0.0.1:8080";

    /// Env override for the bind address. `animus web serve` omits `bind_addr`
    /// from the transport/start config (the plugin falls back to its default
    /// 127.0.0.1:8080), which is unreachable when the transport runs in a
    /// container or behind a private network. This env var lets a deployment
    /// bind 0.0.0.0 without a kernel/CLI change.
    pub const BIND_ENV: &'static str = "ANIMUS_TRANSPORT_HTTP_BIND";

    /// Comma-separated host names to accept besides localhost and loopback
    /// IPs, for deployments that reach the transport under another name.
    /// Declared in the plugin manifest (`env_required`, optional) so the
    /// plugin host forwards it; `BIND_ENV` is deliberately not declared, see
    /// `emit_manifest_if_requested` in main.rs.
    pub const ALLOWED_HOSTS_ENV: &'static str = "ANIMUS_TRANSPORT_ALLOWED_HOSTS";

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
            allowed_hosts: allowed_hosts(
                &config.config,
                std::env::var(Self::ALLOWED_HOSTS_ENV).ok().as_deref(),
            ),
        }
    }
}

/// Union of `config.allowed_hosts` (array of strings) and the comma-separated
/// env value.
fn allowed_hosts(config: &serde_json::Value, env: Option<&str>) -> Vec<String> {
    let from_config = config
        .get("allowed_hosts")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str());
    let from_env = env.into_iter().flat_map(|v| v.split(','));
    let mut hosts: Vec<String> = from_config
        .chain(from_env)
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map(str::to_string)
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

#[cfg(test)]
mod tests {
    use super::allowed_hosts;

    #[test]
    fn allowed_hosts_merges_config_and_env() {
        let config = serde_json::json!({ "allowed_hosts": ["devbox.lan", " "] });
        assert_eq!(
            allowed_hosts(&config, Some("animus.internal, devbox.lan,")),
            vec!["animus.internal".to_string(), "devbox.lan".to_string()]
        );
        assert!(allowed_hosts(&serde_json::Value::Null, None).is_empty());
    }
}
