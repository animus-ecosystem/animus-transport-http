# animus-transport-http

HTTP REST transport plugin for [Animus](https://github.com/launchapp-dev/animus-cli).
Exposes the daemon's control RPC surface as `/api/v1/*` endpoints.

## What it does

`animus-transport-http` runs as a stdio plugin under the Animus plugin host.
At startup the daemon hands it a `TransportConfig` (bind address, control
socket path, project root). It binds an Axum server on that address and
serves a REST surface that maps verbs onto the daemon's control client.

### Route → control method

Every route resolves the daemon control socket, opens a `ControlClient`,
dispatches the matching wire verb, and returns the result wrapped in an
`{"ok": ..., "data"|"error": ...}` envelope.

| Method + path                         | Control method     |
|---------------------------------------|--------------------|
| `GET    /api/v1/daemon/status`        | `daemon_status`    |
| `GET    /api/v1/daemon/health`        | `daemon_health`    |
| `POST   /api/v1/daemon/start`         | `daemon_start` (no-op-OK) |
| `GET    /api/v1/daemon/agents`        | `daemon_agents`    |
| `GET    /api/v1/daemon/logs`          | `daemon_logs`      |
| `DELETE /api/v1/daemon/logs`          | *(501 — no wire verb yet)* |
| `GET    /api/v1/workflows`            | `workflow_list`    |
| `POST   /api/v1/workflows/run`        | `workflow_run`     |
| `POST   /api/v1/workflows/execute`    | `workflow_execute` |
| `GET    /api/v1/workflows/{id}`       | `workflow_get`     |
| `POST   /api/v1/workflows/{id}/pause` | `workflow_pause`   |
| `POST   /api/v1/workflows/{id}/resume`| `workflow_resume`  |
| `POST   /api/v1/workflows/{id}/cancel`| `workflow_cancel`  |
| `GET    /api/v1/queue`                | `queue_list`       |
| `GET    /api/v1/queue/stats`          | `queue_stats`      |
| `POST   /api/v1/queue/enqueue`        | `queue_enqueue`    |
| `POST   /api/v1/queue/reorder`        | `queue_reorder`    |
| `POST   /api/v1/queue/hold/{id}`      | `queue_hold`       |
| `POST   /api/v1/queue/release/{id}`   | `queue_release`    |
| `DELETE /api/v1/queue/drop/{id}`      | `queue_drop`       |
| `GET    /api/v1/plugin/list`          | `plugin_list`      |
| `GET    /api/v1/plugin/info/{name}`   | `plugin_info`      |
| `POST   /api/v1/plugin/install`       | `plugin_install`   |
| `DELETE /api/v1/plugin/uninstall/{name}` | `plugin_uninstall` |
| `POST   /api/v1/plugin/ping/{name}`   | `plugin_ping`      |
| `POST   /api/v1/plugin/call`          | `plugin_call`      |
| `GET    /api/v1/plugin/search`        | `plugin_search`    |
| `GET    /api/v1/plugin/browse`        | `plugin_browse`    |
| `POST   /api/v1/plugin/update`        | `plugin_update`    |
| `GET    /api/v1/subjects?kind=...`    | `subject_list` (requires `kind`) |
| `POST   /api/v1/subjects`             | `subject_create`   |
| `GET    /api/v1/subjects/next?kind=...` | `subject_next` (requires `kind`) |
| `GET    /api/v1/subjects/{id}`        | `subject_get`      |
| `POST   /api/v1/subjects/{id}`        | `subject_update`   |
| `POST   /api/v1/subjects/{id}/status` | `subject_status`   |
| `POST   /api/v1/subject/{plugin}/call`| `plugin_call` (thin subject-backend wrapper) |

This covers the daemon-serveable unary surface of `ControlSurface`. The
remaining control methods are intentionally **not** exposed over HTTP:

- **`daemon_stop` / `daemon_restart`** — the kernel returns `NotSupported`
  over control (it forbids self-termination). Use the CLI:
  `animus daemon stop` / `animus daemon start`.
- **`agent_run` / `agent_status` / `agent_cancel`** — agents are
  in-process / CLI-only; the kernel returns `NotSupported` over control.
  Use `animus agent ...`.
- **`project_init` / `project_setup` / `project_status`** — the project
  surface was removed product-wide.
- **`subject_watch` / `daemon_events` / `workflow_events`** — these are
  streaming subscriptions (see below).

### Streaming

The HTTP transport is **unary-only**. The kernel's streaming subscriptions
(`subject_watch`, `daemon_events`, `workflow_events`) are not exposed here.
Use the GraphQL transport (`animus-transport-graphql`) for subscriptions.

Transport plugins assume the daemon is running. If the control socket cannot
be reached, requests return HTTP 503 with a structured envelope.

## Installation

```bash
animus plugin install animus-transport-http
animus plugin enable animus-transport-http
```

Bind address (default `127.0.0.1:8080`) is configurable per project via
`.animus/config.json`:

```json
{
  "transports": {
    "animus-transport-http": {
      "bind_addr": "127.0.0.1:8080"
    }
  }
}
```

## Layout

```
src/
  main.rs          # transport_backend_main entrypoint
  lib.rs           # re-exports
  backend.rs       # TransportBackend impl
  server.rs        # Axum router + bind
  config.rs        # TransportConfig parsing helpers
  handlers/
    mod.rs           # envelope + connect helpers around animus-control-protocol
    workflows.rs     # workflow_{list,run,execute,get,pause,resume,cancel}
    queue.rs         # queue_{list,stats,enqueue,reorder,hold,release,drop}
    plugin.rs        # plugin_{list,info,install,uninstall,ping,call,search,browse,update}
    daemon.rs        # daemon_{status,health,start,agents,logs}
    subject.rs       # thin /subject/{plugin}/call wrapper over plugin_call
    subject_ops.rs   # typed subject_{list,get,create,update,next,status} CRUD
```

## Development

```bash
cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Pinned to `animus-protocol` `tag = "v0.5.12"`.

## License

[Elastic License 2.0](./LICENSE) — same as upstream Animus.
