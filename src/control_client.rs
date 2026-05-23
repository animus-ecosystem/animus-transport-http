//! Minimal async JSON-RPC client over a Unix-domain control socket.
//!
//! `animus-control-protocol` ships method names, request/response shapes, and
//! the in-process [`animus_control_protocol::ControlSurface`] trait, but not a
//! cross-process client. Transport plugins live in a separate process from the
//! daemon, so this module wraps newline-delimited JSON-RPC frames over the
//! daemon's Unix socket into typed method calls that mirror `ControlSurface`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use animus_plugin_protocol::{RpcError, RpcRequest, RpcResponse};
use anyhow::{anyhow, Context, Result};
use axum::http::StatusCode;
use axum::Json;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

use crate::handlers::error_envelope;

use animus_control_protocol::method;
use animus_control_protocol::types::{
    AgentCancelRequest, AgentRunRequest, AgentRunResult, AgentStatus, AgentStatusRequest,
    DaemonAgentsResponse, DaemonHealthResponse, DaemonLogEntry, DaemonLogsRequest,
    DaemonStatusResponse, PluginBrowseRequest, PluginCallRequest, PluginCallResponse, PluginInfo,
    PluginInfoRequest, PluginInstallRequest, PluginInstallResponse, PluginListRequest,
    PluginListResponse, PluginPingRequest, PluginPingResponse, PluginSearchRequest,
    PluginSearchResponse, PluginUninstallRequest, PluginUpdateRequest, PluginUpdateResponse,
    QueueDropRequest, QueueEnqueueRequest, QueueEntry, QueueHoldRequest, QueueListRequest,
    QueueListResponse, QueueReleaseRequest, QueueReorderRequest, QueueStats, Unit,
    WorkflowCancelRequest, WorkflowGetRequest, WorkflowListRequest, WorkflowListResponse,
    WorkflowPauseRequest, WorkflowResumeRequest, WorkflowRun, WorkflowRunRequest, WorkflowRunStart,
};

pub struct ControlClient {
    stream: Mutex<UnixStream>,
    next_id: AtomicU64,
    socket_path: PathBuf,
}

impl ControlClient {
    pub async fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .await
            .with_context(|| format!("connect {}", socket_path.display()))?;
        Ok(Self {
            stream: Mutex::new(stream),
            next_id: AtomicU64::new(1),
            socket_path: socket_path.to_path_buf(),
        })
    }

    async fn rpc<P: Serialize, R: DeserializeOwned>(&self, method: &str, params: P) -> Result<R> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let params_value = serde_json::to_value(params)
            .with_context(|| format!("serialize params for {method}"))?;
        let request = RpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(Value::from(id)),
            method: method.to_string(),
            params: Some(params_value),
        };
        let mut frame = serde_json::to_string(&request)?;
        frame.push('\n');

        let mut stream = self.stream.lock().await;
        stream
            .write_all(frame.as_bytes())
            .await
            .with_context(|| format!("write {method} to {}", self.socket_path.display()))?;
        stream
            .flush()
            .await
            .with_context(|| format!("flush {method}"))?;

        let mut reader = BufReader::new(&mut *stream);
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = reader
                .read_line(&mut line)
                .await
                .with_context(|| format!("read response for {method}"))?;
            if bytes == 0 {
                return Err(anyhow!(
                    "control socket closed while awaiting response for {method}"
                ));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let response: RpcResponse = serde_json::from_str(trimmed)
                .with_context(|| format!("decode response for {method}"))?;
            if response.id != Some(Value::from(id)) {
                continue;
            }
            if let Some(err) = response.error {
                return Err(rpc_error_to_anyhow(method, err));
            }
            let result = response
                .result
                .ok_or_else(|| anyhow!("{method}: response missing result"))?;
            return serde_json::from_value(result)
                .with_context(|| format!("decode {method} result"));
        }
    }

    async fn rpc_no_params<R: DeserializeOwned>(&self, method: &str) -> Result<R> {
        self.rpc::<Value, R>(method, Value::Null).await
    }

    pub async fn daemon_status(&self) -> Result<DaemonStatusResponse> {
        self.rpc_no_params(method::METHOD_DAEMON_STATUS).await
    }

    pub async fn daemon_health(&self) -> Result<DaemonHealthResponse> {
        self.rpc_no_params(method::METHOD_DAEMON_HEALTH).await
    }

    pub async fn daemon_agents(&self) -> Result<DaemonAgentsResponse> {
        self.rpc_no_params(method::METHOD_DAEMON_AGENTS).await
    }

    pub async fn daemon_logs(
        &self,
        request: DaemonLogsRequest,
        limit: usize,
    ) -> Result<Vec<DaemonLogEntry>> {
        let mut params = serde_json::to_value(&request)?;
        if let Some(obj) = params.as_object_mut() {
            obj.insert("limit".into(), Value::from(limit));
        }
        self.rpc(method::METHOD_DAEMON_LOGS, params).await
    }

    pub async fn workflow_list(
        &self,
        request: WorkflowListRequest,
    ) -> Result<WorkflowListResponse> {
        self.rpc(method::METHOD_WORKFLOW_LIST, request).await
    }

    pub async fn workflow_get(&self, request: WorkflowGetRequest) -> Result<WorkflowRun> {
        self.rpc(method::METHOD_WORKFLOW_GET, request).await
    }

    pub async fn workflow_run(&self, request: WorkflowRunRequest) -> Result<WorkflowRunStart> {
        self.rpc(method::METHOD_WORKFLOW_RUN, request).await
    }

    pub async fn workflow_pause(&self, request: WorkflowPauseRequest) -> Result<Unit> {
        self.rpc(method::METHOD_WORKFLOW_PAUSE, request).await
    }

    pub async fn workflow_resume(&self, request: WorkflowResumeRequest) -> Result<Unit> {
        self.rpc(method::METHOD_WORKFLOW_RESUME, request).await
    }

    pub async fn workflow_cancel(&self, request: WorkflowCancelRequest) -> Result<Unit> {
        self.rpc(method::METHOD_WORKFLOW_CANCEL, request).await
    }

    pub async fn queue_list(&self, request: QueueListRequest) -> Result<QueueListResponse> {
        self.rpc(method::METHOD_QUEUE_LIST, request).await
    }

    pub async fn queue_stats(&self) -> Result<QueueStats> {
        self.rpc_no_params(method::METHOD_QUEUE_STATS).await
    }

    pub async fn queue_enqueue(&self, request: QueueEnqueueRequest) -> Result<QueueEntry> {
        self.rpc(method::METHOD_QUEUE_ENQUEUE, request).await
    }

    pub async fn queue_reorder(&self, request: QueueReorderRequest) -> Result<Unit> {
        self.rpc(method::METHOD_QUEUE_REORDER, request).await
    }

    pub async fn queue_hold(&self, request: QueueHoldRequest) -> Result<Unit> {
        self.rpc(method::METHOD_QUEUE_HOLD, request).await
    }

    pub async fn queue_release(&self, request: QueueReleaseRequest) -> Result<Unit> {
        self.rpc(method::METHOD_QUEUE_RELEASE, request).await
    }

    pub async fn queue_drop(&self, request: QueueDropRequest) -> Result<Unit> {
        self.rpc(method::METHOD_QUEUE_DROP, request).await
    }

    pub async fn plugin_list(&self, request: PluginListRequest) -> Result<PluginListResponse> {
        self.rpc(method::METHOD_PLUGIN_LIST, request).await
    }

    pub async fn plugin_info(&self, request: PluginInfoRequest) -> Result<PluginInfo> {
        self.rpc(method::METHOD_PLUGIN_INFO, request).await
    }

    pub async fn plugin_install(
        &self,
        request: PluginInstallRequest,
    ) -> Result<PluginInstallResponse> {
        self.rpc(method::METHOD_PLUGIN_INSTALL, request).await
    }

    pub async fn plugin_uninstall(&self, request: PluginUninstallRequest) -> Result<Unit> {
        self.rpc(method::METHOD_PLUGIN_UNINSTALL, request).await
    }

    pub async fn plugin_ping(&self, request: PluginPingRequest) -> Result<PluginPingResponse> {
        self.rpc(method::METHOD_PLUGIN_PING, request).await
    }

    pub async fn plugin_call(&self, request: PluginCallRequest) -> Result<PluginCallResponse> {
        self.rpc(method::METHOD_PLUGIN_CALL, request).await
    }

    pub async fn plugin_search(
        &self,
        request: PluginSearchRequest,
    ) -> Result<PluginSearchResponse> {
        self.rpc(method::METHOD_PLUGIN_SEARCH, request).await
    }

    pub async fn plugin_browse(
        &self,
        request: PluginBrowseRequest,
    ) -> Result<PluginSearchResponse> {
        self.rpc(method::METHOD_PLUGIN_BROWSE, request).await
    }

    pub async fn plugin_update(
        &self,
        request: PluginUpdateRequest,
    ) -> Result<PluginUpdateResponse> {
        self.rpc(method::METHOD_PLUGIN_UPDATE, request).await
    }

    pub async fn agent_run(&self, request: AgentRunRequest) -> Result<AgentRunResult> {
        self.rpc(method::METHOD_AGENT_RUN, request).await
    }

    pub async fn agent_status(&self, request: AgentStatusRequest) -> Result<AgentStatus> {
        self.rpc(method::METHOD_AGENT_STATUS, request).await
    }

    pub async fn agent_cancel(&self, request: AgentCancelRequest) -> Result<Unit> {
        self.rpc(method::METHOD_AGENT_CANCEL, request).await
    }
}

fn rpc_error_to_anyhow(method: &str, err: RpcError) -> anyhow::Error {
    anyhow!("{method} failed (code {}): {}", err.code, err.message)
}

pub async fn connect(
    socket_path: &Path,
) -> Result<ControlClient, (StatusCode, Json<serde_json::Value>)> {
    match ControlClient::connect(socket_path).await {
        Ok(client) => Ok(client),
        Err(err) => {
            tracing::warn!(
                error = %err,
                socket = %socket_path.display(),
                "control socket unreachable"
            );
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_envelope(
                    "daemon_unreachable",
                    format!(
                        "could not reach animus daemon at {}: {err}",
                        socket_path.display()
                    ),
                    20,
                )),
            ))
        }
    }
}
