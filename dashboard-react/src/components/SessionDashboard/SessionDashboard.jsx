import React, { useState, useCallback, useEffect } from 'react';
import { LayoutDashboard, Wifi, WifiOff } from 'lucide-react';
import { PayloadUploadZone } from './PayloadUploadZone';
import { UploadSessionList } from './UploadSessionList';
import { AgentSelector } from './AgentSelector';
import { AgentCliPanel } from './AgentCliPanel';
import { FileTransferPanel } from '../FileTransfer';
import { BroadcastModal } from '../BroadcastModal';
import { AgentBuildModal } from '../AgentBuildModal';
import { StatusSummary } from '../StatusIndicator';
import { fetchPayloadSessions } from '../../api/c2Api';
import './session-dashboard.css';

/**
 * Session Dashboard wired to the C2 Rust server (server/src/main.rs).
 *
 * @param {Object} props
 * @param {Array} props.agents - Live agents from useSocket
 * @param {Map} props.commandResults - CommandResult events keyed by command_id
 * @param {Array} [props.payloadUploads] - Live payload uploads from WebSocket
 * @param {boolean} props.isConnected - Dashboard WebSocket status
 */
export function SessionDashboard({
  agents = [],
  commandResults,
  payloadUploads = [],
  isConnected,
  buildEvents = [],
  transferEvents = [],
}) {
  const [uploadSessions, setUploadSessions] = useState([]);
  const [selectedAgentId, setSelectedAgentId] = useState(null);
  const [isPanelOpen, setIsPanelOpen] = useState(false);
  const [showBroadcast, setShowBroadcast] = useState(false);
  const [showBuild, setShowBuild] = useState(false);

  const selectedAgent = agents.find((a) => a.id === selectedAgentId) ?? null;

  // Load persisted upload sessions from C2 server
  useEffect(() => {
    fetchPayloadSessions()
      .then(setUploadSessions)
      .catch(() => setUploadSessions([]));
  }, []);

  // Merge WebSocket payload upload events
  useEffect(() => {
    if (!payloadUploads.length) return;
    setUploadSessions((prev) => {
      const merged = [...payloadUploads];
      for (const session of prev) {
        if (!merged.some((s) => s.id === session.id)) {
          merged.push(session);
        }
      }
      return merged.sort(
        (a, b) => new Date(b.uploadedAt) - new Date(a.uploadedAt),
      );
    });
  }, [payloadUploads]);

  const handleUploadComplete = useCallback((session) => {
    setUploadSessions((prev) => {
      const filtered = prev.filter((s) => s.id !== session.id);
      return [session, ...filtered];
    });
  }, []);

  const handleSelectAgent = useCallback((id) => {
    if (selectedAgentId === id && isPanelOpen) {
      setIsPanelOpen(false);
      setSelectedAgentId(null);
      return;
    }
    setSelectedAgentId(id);
    setIsPanelOpen(true);
  }, [selectedAgentId, isPanelOpen]);

  const handleClosePanel = useCallback(() => {
    setIsPanelOpen(false);
    setSelectedAgentId(null);
  }, []);

  return (
    <div className="session-dashboard flex h-full min-h-0 w-full flex-col bg-slate-950 text-slate-200 lg:flex-row">
      <main className="flex min-h-0 flex-1 flex-col overflow-hidden transition-all duration-300">
        <header className="shrink-0 border-b border-slate-800 bg-slate-900/80 px-4 py-3 backdrop-blur sm:px-6">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="flex items-center gap-2">
                <LayoutDashboard className="h-4 w-4 text-emerald-400" />
                <h1 className="text-sm font-semibold tracking-wide text-slate-100">
                  Session Dashboard
                </h1>
              </div>
              <p className="mt-0.5 text-[11px] text-slate-500">
                C2 server integration — payloads &amp; agent commands
              </p>
            </div>
            <div
              className={[
                'flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] ring-1 ring-inset',
                isConnected
                  ? 'bg-emerald-500/10 text-emerald-300 ring-emerald-500/30'
                  : 'bg-red-500/10 text-red-300 ring-red-500/30',
              ].join(' ')}
            >
              {isConnected ? (
                <Wifi className="h-3 w-3" />
              ) : (
                <WifiOff className="h-3 w-3" />
              )}
              {isConnected ? 'C2 connected' : 'C2 offline'}
            </div>
          </div>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
          <div className="mx-auto grid max-w-4xl gap-5">
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => setShowBroadcast(true)}
                className="rounded-lg border border-sky-500/40 bg-sky-500/10 px-3 py-1.5 text-[11px] text-sky-300 hover:bg-sky-500/20"
              >
                Broadcast to Every Client
              </button>
              <button
                type="button"
                onClick={() => setShowBuild(true)}
                className="rounded-lg border border-emerald-500/40 bg-emerald-500/10 px-3 py-1.5 text-[11px] text-emerald-300 hover:bg-emerald-500/20"
              >
                + New Agent
              </button>
            </div>

            <StatusSummary agents={agents.map((a) => ({ status: a.online ? 'Online' : 'Offline' }))} />

            <PayloadUploadZone onUploadComplete={handleUploadComplete} />
            <UploadSessionList sessions={uploadSessions} />
            <FileTransferPanel agents={agents} transferEvents={transferEvents} />
            {agents.length === 0 ? (
              <div className="rounded-xl border border-dashed border-slate-700 bg-slate-900/40 p-8 text-center">
                <p className="text-sm text-slate-400">No agents registered</p>
                <p className="mt-1 text-xs text-slate-600">
                  Start the server (<code className="text-slate-500">cargo run -p server</code>)
                  and an agent (<code className="text-slate-500">cargo run -p agent</code>)
                </p>
              </div>
            ) : (
              <AgentSelector
                agents={agents}
                selectedAgentId={selectedAgentId}
                onSelectAgent={handleSelectAgent}
              />
            )}
          </div>
        </div>
      </main>

      <AgentCliPanel
        agent={selectedAgent}
        isOpen={isPanelOpen && !!selectedAgent}
        onClose={handleClosePanel}
        commandResults={commandResults}
        isConnected={isConnected}
      />

      <BroadcastModal
        isOpen={showBroadcast}
        onClose={() => setShowBroadcast(false)}
        agents={agents}
      />

      <AgentBuildModal
        isOpen={showBuild}
        onClose={() => setShowBuild(false)}
        buildEvents={buildEvents}
      />
    </div>
  );
}
