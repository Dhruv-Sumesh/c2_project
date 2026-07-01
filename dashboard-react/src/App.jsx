import React, { useState, useMemo } from 'react';
import { useSocket } from './hooks/useSocket';
import { Login } from './components/Login';
import { Sidebar } from './components/Sidebar';
import { Overview } from './components/Overview';
import { AgentDetail } from './components/AgentDetail';
import { Terminal } from './components/Terminal';
import { SessionDashboard } from './components/SessionDashboard';

function App() {
  const [loggedIn, setLoggedIn] = useState(false);
  const [view, setView] = useState('classic');
  const {
    agents,
    logs,
    metricsHistory,
    commandResults,
    payloadUploads,
    isConnected,
  } = useSocket();
  const [selectedAgentId, setSelectedAgentId] = useState(null);

  const sessionAgents = useMemo(
    () =>
      Array.from(agents.values()).map((a) => ({
        id: a.id,
        hostname: a.hostname,
        os: a.os,
        online: a.status === 'Online',
      })),
    [agents],
  );

  if (!loggedIn) {
    return <Login onLogin={() => setLoggedIn(true)} />;
  }

  if (view === 'session') {
    return (
      <div className="flex h-full w-full flex-col">
        <div className="flex shrink-0 items-center justify-end gap-2 border-b border-slate-800 bg-slate-900 px-4 py-2">
          <button className="btn" onClick={() => setView('classic')}>
            classic view
          </button>
          <button className="btn" onClick={() => setLoggedIn(false)}>
            logout
          </button>
        </div>
        <SessionDashboard
          agents={sessionAgents}
          commandResults={commandResults}
          payloadUploads={payloadUploads}
          isConnected={isConnected}
        />
      </div>
    );
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
            <button className="btn" onClick={() => setView('session')}>
              session dashboard
            </button>
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
