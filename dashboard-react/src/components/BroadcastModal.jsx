import React, { useState, useEffect, useCallback } from 'react';
import { X, Radio, GraduationCap } from 'lucide-react';
import { broadcastCommand, fetchBroadcastHistory } from '../api/c2Api';

/**
 * Broadcast command modal — sends a command to all matching agents.
 * Educational feature demonstrating C2 fan-out command delivery.
 */
export function BroadcastModal({ isOpen, onClose, agents = [], onBroadcastSent }) {
  const [command, setCommand] = useState('');
  const [osFilters, setOsFilters] = useState({ windows: true, linux: true, binary: false });
  const [statusFilter, setStatusFilter] = useState('online');
  const [confirming, setConfirming] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const [history, setHistory] = useState([]);

  const loadHistory = useCallback(async () => {
    try {
      const records = await fetchBroadcastHistory();
      setHistory(records);
    } catch {
      setHistory([]);
    }
  }, []);

  useEffect(() => {
    if (isOpen) loadHistory();
  }, [isOpen, loadHistory]);

  const selectedOs = Object.entries(osFilters)
    .filter(([, v]) => v)
    .map(([k]) => k);

  const affectedCount = agents.filter((a) => {
    if (statusFilter === 'online' && !a.online && a.status !== 'Online') return false;
    if (statusFilter === 'offline' && a.online !== false && a.status === 'Online') return false;
    if (selectedOs.length === 0) return true;
    const os = (a.os ?? '').toLowerCase();
    return selectedOs.some((f) => os.includes(f));
  }).length;

  const handleSend = async () => {
    setSending(true);
    setError('');
    try {
      const result = await broadcastCommand(command, {
        os: selectedOs,
        status: statusFilter === 'all' ? ['all'] : [statusFilter],
      });
      onBroadcastSent?.(result);
      setCommand('');
      setConfirming(false);
      loadHistory();
    } catch (e) {
      setError(e.message);
    } finally {
      setSending(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
      <div className="max-h-[90vh] w-full max-w-lg overflow-y-auto rounded-xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center justify-between border-b border-slate-800 px-5 py-4">
          <div className="flex items-center gap-2">
            <Radio className="h-4 w-4 text-sky-400" />
            <h2 className="text-sm font-semibold text-slate-100">Broadcast Command</h2>
          </div>
          <button type="button" onClick={onClose} className="text-slate-500 hover:text-slate-300">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4 p-5">
          <div className="flex items-start gap-2 rounded-lg bg-sky-500/10 px-3 py-2 text-[11px] text-sky-300 ring-1 ring-sky-500/20">
            <GraduationCap className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              Broadcasts queue commands for all matching agents. Delivery occurs on each agent&apos;s next beacon.
            </span>
          </div>

          <label className="block">
            <span className="text-[11px] font-medium text-slate-400">Command</span>
            <input
              type="text"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="whoami"
              className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 font-mono text-sm text-slate-200"
            />
          </label>

          <fieldset>
            <legend className="text-[11px] font-medium text-slate-400">OS Filter</legend>
            <div className="mt-2 flex flex-wrap gap-3">
              {['windows', 'linux', 'binary'].map((os) => (
                <label key={os} className="flex items-center gap-1.5 text-[11px] text-slate-300">
                  <input
                    type="checkbox"
                    checked={osFilters[os]}
                    onChange={(e) => setOsFilters((prev) => ({ ...prev, [os]: e.target.checked }))}
                    className="accent-sky-500"
                  />
                  {os}
                </label>
              ))}
            </div>
          </fieldset>

          <label className="block">
            <span className="text-[11px] font-medium text-slate-400">Status Filter</span>
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 text-sm text-slate-200"
            >
              <option value="online">Online only</option>
              <option value="offline">Offline only</option>
              <option value="all">All agents</option>
            </select>
          </label>

          <p className="text-[11px] text-slate-400">
            Will affect <strong className="text-slate-200">{affectedCount}</strong> agent(s)
          </p>

          {error && (
            <p className="rounded-lg bg-red-500/10 px-3 py-2 text-[11px] text-red-300">{error}</p>
          )}

          {!confirming ? (
            <button
              type="button"
              disabled={!command.trim() || affectedCount === 0}
              onClick={() => setConfirming(true)}
              className="w-full rounded-lg bg-sky-600 py-2.5 text-sm font-medium text-white hover:bg-sky-500 disabled:opacity-50"
            >
              Review Broadcast
            </button>
          ) : (
            <div className="space-y-2">
              <p className="rounded-lg bg-amber-500/10 px-3 py-2 text-[11px] text-amber-300">
                Confirm: send &quot;{command}&quot; to {affectedCount} agent(s)?
              </p>
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => setConfirming(false)}
                  className="flex-1 rounded-lg border border-slate-600 py-2 text-sm text-slate-300"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  onClick={handleSend}
                  disabled={sending}
                  className="flex-1 rounded-lg bg-sky-600 py-2 text-sm font-medium text-white hover:bg-sky-500 disabled:opacity-50"
                >
                  {sending ? 'Sending…' : 'Confirm Broadcast'}
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="border-t border-slate-800 px-5 py-4">
          <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-slate-500">
            Broadcast History
          </h3>
          {history.length === 0 ? (
            <p className="text-[11px] text-slate-600">No broadcasts yet</p>
          ) : (
            <ul className="max-h-32 space-y-1 overflow-y-auto">
              {history.map((r) => (
                <li key={r.id} className="rounded-lg bg-slate-800/50 px-3 py-2 text-[11px]">
                  <span className="font-mono text-slate-300">{r.command}</span>
                  <span className="ml-2 text-slate-500">→ {r.agent_count} agents</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
