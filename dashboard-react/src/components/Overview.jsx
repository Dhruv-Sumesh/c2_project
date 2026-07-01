import React from 'react';

export function Overview({ agents, totalLogsCount }) {
  const agentArray = Array.from(agents.values());
  const onlineCount = agentArray.filter(a => a.status === 'Online').length;

  return (
    <div className="overview">
      <div className="stats-row">
        <div className="stat-item">
          <span className="label">agents</span>
          <span className="value">{agentArray.length}</span>
        </div>
        <div className="stat-item">
          <span className="label">online</span>
          <span className={`value ${onlineCount > 0 ? 'online' : ''}`}>{onlineCount}</span>
        </div>
        <div className="stat-item">
          <span className="label">log events</span>
          <span className="value">{totalLogsCount}</span>
        </div>
      </div>
    </div>
  );
}
