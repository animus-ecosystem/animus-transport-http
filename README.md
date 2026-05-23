# animus-transport-http

HTTP REST transport plugin for [Animus](https://github.com/launchapp-dev/animus-cli).
Exposes the daemon's control RPC surface as `/api/v1/*` endpoints.

## Status

Initial scaffold. Pinned to `animus-protocol` `branch = "main"` until v0.1.5
ships with the `animus-transport-protocol` crate. Re-pin to `tag = "v0.1.5"`
in a follow-up commit before tagging v0.1.0 of this plugin.

## What it does

`animus-transport-http` runs as a stdio plugin under the Animus plugin host.
At startup the daemon hands it a `TransportConfig` (bind address, control
socket path, project root). It binds an Axum server on that address and
serves a REST surface that maps verbs onto the daemon's control client:

| Category   | Examples                                                     |
|------------|--------------------------------------------------------------|
| workflows  | `GET /api/v1/workflows`, `POST /api/v1/workflows/run`, etc.  |
| queue      | `GET /api/v1/queue`, `POST /api/v1/queue/hold/{id}`, etc.    |
| plugin     | `POST /api/v1/plugin/install`, `GET /api/v1/plugin/list`     |
| daemon     | `GET /api/v1/daemon/status`, `GET /api/v1/daemon/health`     |
| subject    | `POST /api/v1/subject/{plugin}/call`                         |
| agent      | `POST /api/v1/agent/run`, `GET /api/v1/agent/{id}/status`    |
| logs       | `GET /api/v1/daemon/logs`                                    |

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
  control_client.rs # connect helper around animus-control-protocol
  handlers/
    mod.rs
    workflows.rs
    queue.rs
    plugin.rs
    daemon.rs
    subject.rs
    agent.rs
```

## Development

```bash
cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

This crate cannot fully compile until
[animus-protocol](https://github.com/launchapp-dev/animus-protocol) v0.1.5
ships the `animus-transport-protocol` and `animus-plugin-runtime`
`transport_backend_main` surfaces. The scaffold is written against the
expected contract.

## License

[Elastic License 2.0](./LICENSE) — same as upstream Animus.
