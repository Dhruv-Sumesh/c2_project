import { useState, useEffect, useCallback } from 'react';
import { getApiBase, getWsUrl } from '../api/c2Api';

export function useSocket() {
  const [agents, setAgents] = useState(new Map());
  const [logs, setLogs] = useState([]);
  const [metricsHistory, setMetricsHistory] = useState(new Map());
  const [commandResults, setCommandResults] = useState(new Map());
  const [payloadUploads, setPayloadUploads] = useState([]);
  const [buildEvents, setBuildEvents] = useState([]);
  const [transferEvents, setTransferEvents] = useState([]);
  const [isConnected, setIsConnected] = useState(false);

  const handleMessage = useCallback((msg) => {
    const payload = msg.payload;

    switch (msg.type) {
      case "InitialAgents":
        setAgents(new Map(payload.map(a => [a.id, a])));
        break;

      case "InitialLogs":
        setLogs(payload.reverse());
        break;

      case "Log":
        setLogs(prev => [...prev, payload]);
        break;

      case "AgentStatus":
        setAgents(prev => {
          const newMap = new Map(prev);
          const existing = newMap.get(payload.id);
          newMap.set(payload.id, {
            ...(existing || {}),
            id: payload.id,
            hostname: payload.hostname ?? existing?.hostname ?? 'unknown',
            os: payload.os ?? existing?.os ?? 'unknown',
            status: payload.status,
            last_seen: payload.last_seen,
          });
          return newMap;
        });
        break;

      case "Metrics":
        setMetricsHistory(prev => {
          const newMap = new Map(prev);
          const history = newMap.get(payload.agent_id) || [];
          const newHistory = [...history, payload];
          if (newHistory.length > 50) newHistory.shift();
          newMap.set(payload.agent_id, newHistory);
          return newMap;
        });
        break;

      case "CommandResult":
        setCommandResults(prev => {
          const next = new Map(prev);
          next.set(payload.command_id, payload);
          return next;
        });
        setLogs(prev => [...prev, {
          level: "INFO",
          source: "Result",
          agent_id: payload.agent_id,
          message: `Command ${payload.command_id.substring(0, 8)} ${payload.status}`,
          timestamp: payload.timestamp,
        }]);
        break;

      case "PayloadUpload":
        setPayloadUploads(prev => {
          const session = {
            id: payload.id,
            fileName: payload.file_name,
            fileSize: payload.file_size,
            status: payload.status,
            uploadedAt: payload.uploaded_at,
          };
          const filtered = prev.filter(s => s.id !== session.id);
          return [session, ...filtered];
        });
        break;

      case "BuildStatus":
        setBuildEvents(prev => [payload, ...prev.filter(e => e.id !== payload.id)]);
        break;

      case "FileTransferProgress":
      case "FileTransferComplete":
        setTransferEvents(prev => [payload, ...prev.slice(0, 49)]);
        break;

      case "BroadcastSent":
        setLogs(prev => [...prev, {
          level: "INFO",
          source: "Broadcast",
          agent_id: null,
          message: `Broadcast sent to ${payload.agent_count} agents: ${payload.command}`,
          timestamp: new Date().toISOString(),
        }]);
        break;

      case "AgentStatusChanged":
        setAgents(prev => {
          const newMap = new Map(prev);
          const existing = newMap.get(payload.agent_id);
          if (existing) {
            newMap.set(payload.agent_id, {
              ...existing,
              status: payload.new_status,
            });
          }
          return newMap;
        });
        break;

      default:
        break;
    }
  }, []);

  useEffect(() => {
    // Use WSS when the page is served over HTTPS (production build on the C2 server).
    let wsUri = window.location.protocol === "https:" ? "wss:" : "ws:";

    if (window.location.port === "5173") {
      // Vite dev server proxies /api to https://localhost:3443
      wsUri = "wss://localhost:3443/api/dashboard/ws";
    } else if (window.location.host) {
      wsUri += `//${window.location.host}/api/dashboard/ws`;
    } else {
      wsUri = "wss://localhost:3443/api/dashboard/ws";
    }

    const socket = new WebSocket(wsUri);

    socket.onopen = () => setIsConnected(true);
    socket.onclose = () => setIsConnected(false);

    socket.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        handleMessage(msg);
      } catch (e) {
        console.error("Error processing WS message:", e);
      }
    };

    // Poll agents as backup when WebSocket events are delayed (beacon polling).
    const apiBase = window.location.port === "5173"
      ? "https://localhost:3443"
      : window.location.origin;

    const poll = async () => {
      try {
        const res = await fetch(`${apiBase}/api/agents`);
        if (res.ok) {
          const agentList = await res.json();
          if (Array.isArray(agentList)) {
            setAgents(new Map(agentList.map(a => [a.id, a])));
          }
        }
      } catch {
        // server may be down
      }
    };

    poll();
    const pollInterval = setInterval(poll, 15000);

    return () => {
      clearInterval(pollInterval);
      socket.close();
    };
  }, [handleMessage]);

  return { agents, logs, metricsHistory, commandResults, payloadUploads, buildEvents, transferEvents, isConnected };
}
