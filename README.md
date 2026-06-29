# Educational Multi-Agent C2 Simulator

A local simulator for studying multi-agent registration, authentication, WebSocket messaging, heartbeats, reconnection, logging, and dashboard architecture.

**Not implemented (by design):** remote shell, arbitrary command execution, file transfer, or code deployment.

## Prerequisites

- [Rust](https://rustup.rs/) (1.70+)

## Quick start

Open **two terminals** from the project root.

**Terminal 1 — start the server** (serves the dashboard at http://localhost:3000):

```bash
chmod +x scripts/run-server.sh scripts/run-agent.sh
./scripts/run-server.sh
```

**Terminal 2 — start an agent**:

```bash
./scripts/run-agent.sh
```

Open http://localhost:3000 in your browser. You should see:

- The agent appear in the sidebar as **Online**
- Live CPU / memory / disk metrics updating
- Log events streaming in the console

To run a second agent, open a third terminal and run `./scripts/run-agent.sh` again (each instance gets its own `agent_id.txt` if started from a different directory, or delete `agent_id.txt` first to simulate a new host).

## Manual run (without scripts)

```bash
# Terminal 1
cargo run -p server

# Terminal 2
cargo run -p agent
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `C2_SERVER_WS_URL` | `ws://localhost:3000/api/agent/ws` | Agent WebSocket endpoint |
| `C2_PSK` | `educational-c2-psk-key` | Shared secret for challenge-response auth |

Both server and agent must use the same `C2_PSK`.

## Architecture

```
┌─────────────┐     WebSocket      ┌─────────────┐     WebSocket      ┌─────────────┐
│   Agent(s)  │ ◄────────────────► │   Server    │ ◄────────────────► │  Dashboard  │
│  (Rust)     │  register/auth/    │  (Axum)     │  live events       │  (HTML/JS)  │
│             │  heartbeat/metrics │  + SQLite   │                    │             │
└─────────────┘                    └─────────────┘                    └─────────────┘
```

### Agent handshake

1. Agent sends `Register` with ID, hostname, OS
2. Server replies with `Challenge` (nonce)
3. Agent sends `Proof` (HMAC-SHA256 of nonce with PSK)
4. Server replies with `AuthOk` or `AuthFail`
5. Agent streams `Heartbeat` (every 5s) and `SystemInfo` metrics (every 3s)

## API

| Endpoint | Description |
|----------|-------------|
| `GET /` | Dashboard UI |
| `GET /api/agents` | List registered agents |
| `GET /api/agents/:id/metrics` | Metrics history |
| `GET /api/agents/:id/logs` | Agent-specific logs |
| `GET /api/logs` | Global log stream |
| `WS /api/agent/ws` | Agent gateway |
| `WS /api/dashboard/ws` | Dashboard live events |

## Project layout

```
├── agent/          Rust agent simulator
├── server/         Axum server + SQLite persistence
├── dashboard/      Static web UI
└── scripts/        Helper run scripts
```
