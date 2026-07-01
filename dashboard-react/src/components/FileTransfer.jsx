import React, { useState, useCallback, useEffect, useRef } from 'react';
import { Upload, FileText, CheckCircle, AlertCircle } from 'lucide-react';
import { uploadFileToAgent, fetchAgentTransfers } from '../api/c2Api';
import { StatusIndicator } from './StatusIndicator';

/**
 * Per-agent drag-and-drop file transfer zone.
 * Uploads files to the C2 server which queues delivery to the agent via beacon.
 */
export function FileTransferZone({ agent, transferEvents = [] }) {
  const [dragOver, setDragOver] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState('');
  const [transfers, setTransfers] = useState([]);
  const [destPath, setDestPath] = useState('');
  const inputRef = useRef(null);

  const loadTransfers = useCallback(async () => {
    if (!agent?.id) return;
    try {
      const list = await fetchAgentTransfers(agent.id);
      setTransfers(list);
    } catch {
      setTransfers([]);
    }
  }, [agent?.id]);

  useEffect(() => {
    loadTransfers();
  }, [loadTransfers]);

  useEffect(() => {
    const relevant = transferEvents.filter((e) => e.agent_id === agent?.id);
    if (relevant.length) loadTransfers();
  }, [transferEvents, agent?.id, loadTransfers]);

  const handleFiles = async (files) => {
    if (!files?.length || !agent?.id) return;
    const file = files[0];
    setError('');
    setUploading(true);
    setProgress(0);

    try {
      await uploadFileToAgent(agent.id, file, destPath || file.name, setProgress);
      await loadTransfers();
    } catch (e) {
      setError(e.message);
    } finally {
      setUploading(false);
      setProgress(0);
    }
  };

  const onDrop = (e) => {
    e.preventDefault();
    setDragOver(false);
    handleFiles(e.dataTransfer.files);
  };

  const statusLabel = agent?.status ?? (agent?.online ? 'Online' : 'Offline');

  return (
    <div className="rounded-xl border border-slate-700/80 bg-slate-900/60 p-4">
      <div className="mb-3 flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-slate-200">{agent.hostname}</p>
          <p className="text-[10px] text-slate-500">{agent.os}</p>
        </div>
        <StatusIndicator status={statusLabel} lastSeen={agent.last_seen} />
      </div>

      <div
        role="button"
        tabIndex={0}
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={onDrop}
        onClick={() => inputRef.current?.click()}
        onKeyDown={(e) => e.key === 'Enter' && inputRef.current?.click()}
        className={[
          'cursor-pointer rounded-lg border-2 border-dashed p-6 text-center transition',
          dragOver
            ? 'border-emerald-400 bg-emerald-500/10'
            : 'border-slate-700 hover:border-slate-500 hover:bg-slate-800/40',
        ].join(' ')}
      >
        <Upload className="mx-auto h-5 w-5 text-slate-500" />
        <p className="mt-2 text-[11px] text-slate-400">
          Drop file to send to this agent
        </p>
        <input
          ref={inputRef}
          type="file"
          className="hidden"
          onChange={(e) => handleFiles(e.target.files)}
        />
      </div>

      <input
        type="text"
        value={destPath}
        onChange={(e) => setDestPath(e.target.value)}
        placeholder="Destination path (optional)"
        className="mt-2 w-full rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 text-[11px] text-slate-300"
      />

      {uploading && (
        <div className="mt-3">
          <div className="h-1.5 overflow-hidden rounded-full bg-slate-800">
            <div
              className="h-full bg-emerald-500 transition-all"
              style={{ width: `${progress}%` }}
            />
          </div>
          <p className="mt-1 text-[10px] text-slate-500">Uploading to server… {progress}%</p>
        </div>
      )}

      {error && (
        <p className="mt-2 flex items-center gap-1 text-[11px] text-red-400">
          <AlertCircle className="h-3 w-3" /> {error}
        </p>
      )}

      {transfers.length > 0 && (
        <ul className="mt-3 space-y-1.5">
          {transfers.slice(0, 5).map((t) => {
            const pct = t.chunks_total > 0
              ? Math.round((t.chunks_received / t.chunks_total) * 100)
              : t.status === 'completed' ? 100 : 0;
            return (
              <li key={t.id} className="rounded-lg bg-slate-800/50 px-3 py-2">
                <div className="flex items-center gap-2">
                  <FileText className="h-3 w-3 text-slate-500" />
                  <span className="flex-1 truncate text-[11px] text-slate-300">{t.file_path}</span>
                  {t.status === 'completed' ? (
                    <CheckCircle className="h-3 w-3 text-emerald-400" />
                  ) : (
                    <span className="text-[10px] text-slate-500">{t.status}</span>
                  )}
                </div>
                {t.status === 'in_progress' && (
                  <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-slate-700">
                    <div className="h-full bg-sky-500 transition-all" style={{ width: `${pct}%` }} />
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

/** Grid of per-agent file transfer zones */
export function FileTransferPanel({ agents = [], transferEvents = [] }) {
  if (!agents.length) return null;

  return (
    <div className="rounded-xl border border-slate-700/80 bg-slate-900/40 p-5">
      <h3 className="mb-4 text-sm font-semibold text-slate-100">File Transfers</h3>
      <p className="mb-4 text-[11px] text-slate-500">
        Drag files onto an agent zone to queue delivery via encrypted beacon channel (64KB chunks, SHA-256 verified).
      </p>
      <div className="grid gap-4 sm:grid-cols-2">
        {agents.map((agent) => (
          <FileTransferZone
            key={agent.id}
            agent={agent}
            transferEvents={transferEvents}
          />
        ))}
      </div>
    </div>
  );
}
