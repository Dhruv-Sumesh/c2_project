import React from 'react';
import { MetricsChart } from './MetricsChart';

function fmt(isoStr) {
  if (!isoStr) return '—';
  try {
    return new Date(isoStr).toISOString().replace('T', ' ').substring(0, 19);
  } catch {
    return isoStr;
  }
}

export function AgentDetail({ agent, metricsHistory = [] }) {
  if (!agent) return null;

  const m = metricsHistory.length > 0
    ? metricsHistory[metricsHistory.length - 1]
    : { cpu_usage: 0, memory_usage: 0, disk_usage: 0 };

  const isOnline = agent.status === 'Online';

  return (
    <div className="agent-detail">
      <div className="detail-row">
        {/* Left: profile info */}
        <div className="detail-col">
          <div className="detail-section-title">agent info</div>
          <div className="info-table">
            <span className="k">hostname</span>
            <span className="v">{agent.hostname}</span>
            <span className="k">id</span>
            <span className="v">{agent.id.substring(0, 16)}…</span>
            <span className="k">os</span>
            <span className="v">{agent.os}</span>
            <span className="k">status</span>
            <span className="v">
              <span className={`badge ${isOnline ? 'online' : 'offline'}`}>
                {agent.status.toLowerCase()}
              </span>
            </span>
            <span className="k">last seen</span>
            <span className="v">{fmt(agent.last_seen)}</span>
          </div>
        </div>

        {/* Right: live metrics */}
        <div className="detail-col">
          <div className="detail-section-title">live telemetry</div>
          <div className="metric-row">
            <span className="metric-label">cpu</span>
            <div className="bar-track">
              <div className="bar-fill cpu" style={{ width: `${m.cpu_usage}%` }} />
            </div>
            <span className="metric-pct">{m.cpu_usage.toFixed(1)}%</span>
          </div>
          <div className="metric-row">
            <span className="metric-label">mem</span>
            <div className="bar-track">
              <div className="bar-fill mem" style={{ width: `${m.memory_usage}%` }} />
            </div>
            <span className="metric-pct">{m.memory_usage.toFixed(1)}%</span>
          </div>
          <div className="metric-row">
            <span className="metric-label">disk</span>
            <div className="bar-track">
              <div className="bar-fill disk" style={{ width: `${m.disk_usage}%` }} />
            </div>
            <span className="metric-pct">{m.disk_usage.toFixed(1)}%</span>
          </div>
        </div>
      </div>

      <MetricsChart data={metricsHistory} />
    </div>
  );
}
