# Educational Multi-Agent C2 Simulator

A local simulator for studying multi-agent registration, authentication, HTTPS beacon messaging, encrypted session keys, configurable polling intervals, reconnection, logging, and dashboard architecture.

**Not implemented (by design):** remote shell, arbitrary command execution, file transfer, or code deployment.

## Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (for building the React dashboard)

## Quick start

Open **two terminals** from the project root.

**Terminal 1 — start the server** (serves the dashboard at https://localhost:3443):

```bash
chmod +x scripts/run-server.sh scripts/run-agent.sh
./scripts/run-server.sh
```

**Terminal 2 — start an agent**:

```bash
./scripts/run-agent.sh
```

Open https://localhost:3443 in your browser (accept the self-signed certificate warning in development). You should see:

- The agent appear in the sidebar as **Online**
- Live CPU / memory / disk metrics updating
- Log events streaming in the console

To run a second agent, open a third terminal and run `./scripts/run-agent.sh` again (each instance gets its own `agent_id.txt` if started from a different directory, or delete `agent_id.txt` first to simulate a new host).

## Manual run (without scripts)

```bash
# Terminal 1 — build dashboard first
cd dashboard-react && npm install && npm run build && cd ..

# Terminal 1 — start server (HTTPS on port 3443)
cargo run -p server

# Terminal 2 — start agent
cargo run -p agent
```

### Windows (PowerShell)

```powershell
cd dashboard-react; npm install; npm run build; cd ..
cargo run -p server
# In a second terminal:
$env:C2_SERVER_URL="https://localhost:3443"
cargo run -p agent
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `C2_SERVER_URL` | `https://localhost:3443` | Agent HTTPS base URL |
| `C2_PSK` | `educational-c2-psk-key` | Shared secret for bootstrap auth token |
| `C2_BEACON_INTERVAL` | `30` | Initial agent polling interval (seconds) |
| `C2_CERT_DIR` | `certs` | Directory for auto-generated TLS certificates |

Both server and agent must use the same `C2_PSK`.

## Security features

### HTTPS / TLS

All agent-server communication uses **HTTPS** on port **3443**. The server auto-generates a self-signed certificate in `certs/` on first run. The agent accepts self-signed certs in development (`danger_accept_invalid_certs`).

### Session key exchange

1. First beacon uses the PSK-derived key (`bootstrap: true`) only to establish a session.
2. Server generates a random 256-bit session key and returns it in the encrypted response.
3. Agent stores the session key **in memory only** (not on disk).
4. All subsequent beacons and command results use the session key.
5. On `401 Unauthorized` or decrypt failure, the agent clears the session key and re-bootstraps with the PSK.

### Configurable sleep interval

- Each agent polls the server on a configurable interval (default 30 seconds).
- The server persists per-agent intervals in SQLite.
- Queue a `set_interval` command to change an agent's polling interval:

```bash
./scripts/queue-command.sh $(cat agent_id.txt) 45 set_interval
```

- Valid range: **5–3600** seconds. The server validates updates; the agent validates the value in each beacon response.

## Architecture

```
┌─────────────┐     HTTPS/TLS      ┌─────────────┐     WSS (TLS)      ┌─────────────┐
│   Agent(s)  │ ◄────────────────► │   Server    │ ◄────────────────► │  Dashboard  │
│  (Rust)     │  encrypted beacon  │  (Axum)     │  live events       │  (React)    │
│             │  + session keys    │  + SQLite   │                    │             │
└─────────────┘                    └─────────────┘                    └─────────────┘
```

### Agent beacon flow

1. Agent sends encrypted `POST /api/beacon` (bootstrap with PSK on first connect)
2. Server replies with encrypted response including optional `session_key` and `sleep_interval_secs`
3. Agent applies session key (memory) and polling interval
4. Server may include queued commands in the response
5. Agent executes commands and posts encrypted results to `POST /api/result`

## API

| Endpoint | Description |
|----------|-------------|
| `GET /` | Dashboard UI |
| `GET /api/agents` | List registered agents |
| `GET /api/agents/:id/metrics` | Metrics history |
| `GET /api/agents/:id/logs` | Agent-specific logs |
| `GET /api/agents/:id/results` | Command results |
| `GET /api/logs` | Global log stream |
| `POST /api/beacon` | Encrypted agent beacon (AES-GCM) |
| `POST /api/result` | Encrypted command result |
| `POST /api/command/queue` | Queue a command for an agent |
| `WS /api/dashboard/ws` | Dashboard live events (WSS over TLS) |

## Project layout

```
├── agent/              Rust HTTPS beacon agent
├── server/             Axum HTTPS server + SQLite persistence
├── dashboard-react/    React web UI
├── certs/              Auto-generated TLS certificates (dev)
└── scripts/            Helper run scripts
```
