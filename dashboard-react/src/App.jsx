import React, { useState } from 'react';
import { useSocket } from './hooks/useSocket';
import { Login } from './components/Login';
import { Sidebar } from './components/Sidebar';
import { Overview } from './components/Overview';
import { AgentDetail } from './components/AgentDetail';
import { Terminal } from './components/Terminal';

function App() {
  const [loggedIn, setLoggedIn] = useState(false);
  const { agents, logs, metricsHistory, isConnected } = useSocket();
  const [selectedAgentId, setSelectedAgentId] = useState(null);

  if (!loggedIn) {
    return <Login onLogin={() => setLoggedIn(true)} />;
  }

  const handleSelectAgent = (id) => {
    setSelectedAgentId(prev => prev === id ? null : id);
  };

  const selectedAgent = selectedAgentId ? agents.get(selectedAgentId) : null;

  return (
    <>
      <Sidebar
        agents={agents}
        isConnected={isConnected}
        selectedAgentId={selectedAgentId}
        onSelectAgent={handleSelectAgent}
      />

      <div className="main-content">
        <div className="topbar">
          <div className="topbar-left">
            <span className="topbar-title">
              {selectedAgent ? selectedAgent.hostname : 'overview'}
            </span>
            <span className="topbar-sub">
              {selectedAgent
                ? `id: ${selectedAgent.id.substring(0, 12)}…`
                : 'all agents'}
            </span>
          </div>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <button className="btn" onClick={() => window.location.reload()}>
              refresh
            </button>
            <button className="btn" onClick={() => setLoggedIn(false)}>
              logout
            </button>
          </div>
        </div>

        <div className="body-area">
          {selectedAgent ? (
            <AgentDetail
              agent={selectedAgent}
              metricsHistory={metricsHistory.get(selectedAgentId) || []}
            />
          ) : (
            <Overview agents={agents} totalLogsCount={logs.length} />
          )}

          <Terminal logs={logs} selectedAgentId={selectedAgentId} />
        </div>
      </div>
    </>
  );
}

export default App;
