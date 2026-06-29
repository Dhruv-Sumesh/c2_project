import React from 'react';

export function Sidebar({ agents, isConnected, selectedAgentId, onSelectAgent }) {
  const agentArray = Array.from(agents.values());

  return (
    <aside className="sidebar">
      <div className="sidebar-header">c2-simulator</div>

      <div className="server-status">
        <div className={`status-indicator ${isConnected ? '' : 'offline'}`}>
          <span className="status-dot" />
          {isConnected ? 'connected' : 'disconnected'}
        </div>
        <p>port <code>3000</code> &middot; sqlite</p>
      </div>

      <div className="agent-list-title">agents</div>

      <ul className="agent-list">
        {agentArray.length === 0 ? (
          <li style={{ padding: '8px 12px', color: '#555', fontSize: '11px' }}>
            no agents registered
          </li>
        ) : (
          agentArray.map(agent => (
            <li
              key={agent.id}
              className={`agent-item ${selectedAgentId === agent.id ? 'active' : ''}`}
              onClick={() => onSelectAgent(agent.id)}
            >
              <div className="agent-info">
                <h4>{agent.hostname}</h4>
                <p>
                  <span
                    className="status-dot"
                    style={{ background: agent.status === 'Online' ? 'var(--green)' : 'var(--red)' }}
                  />
                  {agent.status.toLowerCase()} &middot; {agent.os}
                </p>
              </div>
            </li>
          ))
        )}
      </ul>
    </aside>
  );
}
