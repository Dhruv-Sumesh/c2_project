import React, { useState } from 'react';
import { Plus, Radio } from 'lucide-react';
import { StatusIndicator, StatusSummary } from './StatusIndicator';
import { AgentBuildModal } from './AgentBuildModal';
import { BroadcastModal } from './BroadcastModal';

export function Sidebar({
  agents,
  isConnected,
  selectedAgentId,
  onSelectAgent,
  buildEvents = [],
  onBroadcastSent,
}) {
  const agentArray = Array.from(agents.values());
  const [showBuildModal, setShowBuildModal] = useState(false);
  const [showBroadcastModal, setShowBroadcastModal] = useState(false);
  const [statusFilter, setStatusFilter] = useState('all');

  const filteredAgents = agentArray.filter((a) => {
    if (statusFilter === 'all') return true;
    return (a.status ?? '').toLowerCase() === statusFilter;
  });

  const sessionAgents = agentArray.map((a) => ({
    id: a.id,
    hostname: a.hostname,
    os: a.os,
    status: a.status,
    online: a.status === 'Online',
    last_seen: a.last_seen,
  }));

  return (
    <>
      <aside className="sidebar">
        <div className="sidebar-header">c2-simulator</div>

        <div className="server-status">
          <div className={`status-indicator ${isConnected ? '' : 'offline'}`}>
            <span className="status-dot" />
            {isConnected ? 'connected' : 'disconnected'}
          </div>
          <p>port <code>3443</code> &middot; sqlite</p>
        </div>

        <div className="px-3 py-2">
          <button
            type="button"
            className="btn w-full flex items-center justify-center gap-1.5 text-[11px]"
            onClick={() => setShowBroadcastModal(true)}
            style={{ marginBottom: '6px' }}
          >
            <Radio size={12} />
            Broadcast to Clients
          </button>
        </div>

        <div className="agent-list-title flex items-center justify-between px-3">
          <span>agents</span>
          <button
            type="button"
            onClick={() => setShowBuildModal(true)}
            className="flex items-center gap-0.5 text-[10px] text-emerald-400 hover:text-emerald-300"
            title="Build new agent binary"
          >
            <Plus size={12} />
            New Agent
          </button>
        </div>

        <div className="px-3 pb-2">
          <StatusSummary agents={agentArray} />
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            className="mt-2 w-full rounded border border-[#333] bg-[#1a1a1a] px-2 py-1 text-[10px] text-[#aaa]"
          >
            <option value="all">All statuses</option>
            <option value="online">Online</option>
            <option value="offline">Offline</option>
          </select>
        </div>

        <ul className="agent-list">
          {filteredAgents.length === 0 ? (
            <li style={{ padding: '8px 12px', color: '#555', fontSize: '11px' }}>
              no agents registered
            </li>
          ) : (
            filteredAgents.map((agent) => (
              <li
                key={agent.id}
                className={`agent-item ${selectedAgentId === agent.id ? 'active' : ''}`}
                onClick={() => onSelectAgent(agent.id)}
              >
                <div className="agent-info">
                  <h4>{agent.hostname}</h4>
                  <p className="flex items-center gap-1.5">
                    <StatusIndicator
                      status={agent.status ?? 'Unknown'}
                      lastSeen={agent.last_seen}
                      showLabel={false}
                    />
                    <span style={{ fontSize: '10px', color: '#666' }}>
                      {agent.status?.toLowerCase()} &middot; {agent.os}
                    </span>
                  </p>
                </div>
              </li>
            ))
          )}
        </ul>
      </aside>

      <AgentBuildModal
        isOpen={showBuildModal}
        onClose={() => setShowBuildModal(false)}
        buildEvents={buildEvents}
      />

      <BroadcastModal
        isOpen={showBroadcastModal}
        onClose={() => setShowBroadcastModal(false)}
        agents={sessionAgents}
        onBroadcastSent={onBroadcastSent}
      />
    </>
  );
}
