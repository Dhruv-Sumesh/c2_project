import { useState, useEffect, useCallback } from 'react';

export function useSocket() {
  const [agents, setAgents] = useState(new Map());
  const [logs, setLogs] = useState([]);
  const [metricsHistory, setMetricsHistory] = useState(new Map());
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    let wsUri = window.location.protocol === "https:" ? "wss:" : "ws:";
    
    // Check if we're running locally via Vite dev server
    if (window.location.port === "5173") {
      wsUri = "ws://localhost:3000/api/dashboard/ws";
    } else if (window.location.host) {
      wsUri += `//${window.location.host}/api/dashboard/ws`;
    } else {
      wsUri = "ws://localhost:3000/api/dashboard/ws";
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

    return () => socket.close();
  }, []);

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
          const agent = newMap.get(payload.id);
          if (agent) {
            newMap.set(payload.id, { ...agent, status: payload.status, last_seen: payload.last_seen });
          } else {
            // New agent logic would normally trigger a full refresh here
          }
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
      
      default:
        break;
    }
  }, []);

  return { agents, logs, metricsHistory, isConnected };
}
