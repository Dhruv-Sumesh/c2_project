import React, { useState, useEffect, useCallback } from 'react';
import { X, Download, Hammer, RefreshCw, GraduationCap } from 'lucide-react';
import {
  buildAgent,
  fetchAgentBuilds,
  generatePsk,
  getAgentDownloadUrl,
  getDefaultServerUrl,
} from '../api/c2Api';

/**
 * Educational agent build modal — compiles cross-platform agent binaries
 * with embedded C2 configuration (server URL, PSK, beacon interval).
 */
export function AgentBuildModal({ isOpen, onClose, buildEvents = [] }) {
  const [targetOs, setTargetOs] = useState('windows');
  const [serverUrl, setServerUrl] = useState(getDefaultServerUrl());
  const [psk, setPsk] = useState('');
  const [beaconInterval, setBeaconInterval] = useState(30);
  const [building, setBuilding] = useState(false);
  const [error, setError] = useState('');
  const [builds, setBuilds] = useState([]);
  const [activeBuildId, setActiveBuildId] = useState(null);

  const loadBuilds = useCallback(async () => {
    try {
      const list = await fetchAgentBuilds();
      setBuilds(list);
    } catch {
      setBuilds([]);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      setPsk(generatePsk());
      loadBuilds();
    }
  }, [isOpen, loadBuilds]);

  useEffect(() => {
    if (!buildEvents.length) return;
    const latest = buildEvents[0];
    if (latest?.id) {
      setActiveBuildId(latest.id);
      if (latest.status === 'completed' || latest.status === 'failed') {
        setBuilding(false);
        if (latest.status === 'failed') {
          setError(latest.error || 'Build failed');
        } else {
          setError('');
        }
        loadBuilds();
      }
    }
  }, [buildEvents, loadBuilds]);

  const handleBuild = async () => {
    setError('');
    setBuilding(true);
    try {
      const result = await buildAgent({
        targetOs,
        serverUrl,
        psk,
        beaconInterval,
      });
      setActiveBuildId(result.build_id);
    } catch (e) {
      setError(e.message);
      setBuilding(false);
    }
  };

  if (!isOpen) return null;

  const activeBuild = builds.find((b) => b.id === activeBuildId);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
      <div className="max-h-[90vh] w-full max-w-lg overflow-y-auto rounded-xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center justify-between border-b border-slate-800 px-5 py-4">
          <div className="flex items-center gap-2">
            <Hammer className="h-4 w-4 text-amber-400" />
            <h2 className="text-sm font-semibold text-slate-100">New Agent Build</h2>
          </div>
          <button type="button" onClick={onClose} className="text-slate-500 hover:text-slate-300">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4 p-5">
          <div className="flex items-start gap-2 rounded-lg bg-violet-500/10 px-3 py-2 text-[11px] text-violet-300 ring-1 ring-violet-500/20">
            <GraduationCap className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>
              Educational use only. Builds compile the Rust agent with embedded config for lab environments.
            </span>
          </div>

          <label className="block">
            <span className="text-[11px] font-medium text-slate-400">Target OS</span>
            <select
              value={targetOs}
              onChange={(e) => setTargetOs(e.target.value)}
              className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 text-sm text-slate-200"
            >
              <option value="windows">Windows (.exe)</option>
              <option value="linux">Linux x86_64 (ELF)</option>
              <option value="linux-arm64">Linux ARM64 / Kali ARM (ELF)</option>
              <option value="linux-arm32">Linux ARM32 / RPi (ELF)</option>
              <option value="binary">Native Binary (.bin)</option>
            </select>
          </label>

          <label className="block">
            <span className="text-[11px] font-medium text-slate-400">C2 Server URL</span>
            <input
              type="url"
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
              className="mt-1 w-full rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 text-sm text-slate-200"
              placeholder="https://localhost:3443"
            />
          </label>

          <label className="block">
            <span className="text-[11px] font-medium text-slate-400">Pre-Shared Key (PSK)</span>
            <div className="mt-1 flex gap-2">
              <input
                type="text"
                value={psk}
                onChange={(e) => setPsk(e.target.value)}
                className="flex-1 rounded-lg border border-slate-700 bg-slate-800 px-3 py-2 font-mono text-xs text-slate-200"
              />
              <button
                type="button"
                onClick={() => setPsk(generatePsk())}
                className="rounded-lg border border-slate-600 px-3 text-[11px] text-slate-300 hover:bg-slate-800"
              >
                Generate
              </button>
            </div>
          </label>

          <label className="block">
            <span className="text-[11px] font-medium text-slate-400">
              Beacon Interval: {beaconInterval}s
            </span>
            <input
              type="range"
              min={5}
              max={60}
              value={beaconInterval}
              onChange={(e) => setBeaconInterval(Number(e.target.value))}
              className="mt-2 w-full accent-emerald-500"
            />
          </label>

          {error && (
            <p className="rounded-lg bg-red-500/10 px-3 py-2 text-[11px] text-red-300">{error}</p>
          )}

          {building && (
            <div className="flex items-center gap-2 rounded-lg bg-amber-500/10 px-3 py-2 text-[11px] text-amber-300">
              <RefreshCw className="h-3 w-3 animate-spin" />
              Compiling agent… this may take a minute.
            </div>
          )}

          <button
            type="button"
            onClick={handleBuild}
            disabled={building}
            className="w-full rounded-lg bg-emerald-600 py-2.5 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-50"
          >
            {building ? 'Building…' : 'Build Agent'}
          </button>

          {activeBuild?.status === 'completed' && (
            <a
              href={getAgentDownloadUrl(activeBuild.id)}
              className="flex w-full items-center justify-center gap-2 rounded-lg border border-emerald-500/40 bg-emerald-500/10 py-2.5 text-sm text-emerald-300 hover:bg-emerald-500/20"
            >
              <Download className="h-4 w-4" />
              Download Latest Build
            </a>
          )}
        </div>

        <div className="border-t border-slate-800 px-5 py-4">
          <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-slate-500">
            Build History
          </h3>
          {builds.length === 0 ? (
            <p className="text-[11px] text-slate-600">No builds yet</p>
          ) : (
            <ul className="max-h-40 space-y-1 overflow-y-auto">
              {builds.map((b) => (
                <li
                  key={b.id}
                  className="flex flex-col gap-1 rounded-lg bg-slate-800/50 px-3 py-2 text-[11px]"
                >
                  <div className="flex items-center justify-between">
                    <div>
                      <span className="font-medium text-slate-300">{b.target_os}</span>
                      <span className="ml-2 text-slate-500">{b.status}</span>
                    </div>
                    {b.status === 'completed' && (
                      <a
                        href={getAgentDownloadUrl(b.id)}
                        className="text-emerald-400 hover:underline"
                      >
                        Download
                      </a>
                    )}
                  </div>
                  {b.error && (
                    <div className="mt-1 max-h-24 overflow-y-auto rounded bg-red-950/40 p-1.5 font-mono text-[9px] text-red-300 border border-red-900/30 whitespace-pre-wrap">
                      {b.error}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
