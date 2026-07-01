import React from 'react';
import { Bot, ChevronRight } from 'lucide-react';
import { StatusIndicator } from '../StatusIndicator';

/** Clickable agent list — opens the CLI side panel for the selected agent */
export function AgentSelector({ agents, selectedAgentId, onSelectAgent }) {
  return (
    <div className="rounded-xl border border-slate-700/80 bg-slate-900/60 p-5 shadow-lg shadow-black/20">
      <div className="mb-4 flex items-center gap-2">
        <Bot className="h-4 w-4 text-violet-400" />
        <h3 className="text-sm font-semibold text-slate-100">Active Agents</h3>
      </div>

      <ul className="space-y-1.5">
        {agents.map((agent) => {
          const isSelected = selectedAgentId === agent.id;
          const status = agent.status ?? (agent.online ? 'Online' : 'Offline');
          return (
            <li key={agent.id}>
              <button
                type="button"
                onClick={() => onSelectAgent(agent.id)}
                className={[
                  'group flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition',
                  isSelected
                    ? 'border-violet-500/50 bg-violet-500/10'
                    : 'border-transparent bg-slate-800/30 hover:border-slate-600 hover:bg-slate-800/60',
                ].join(' ')}
              >
                <StatusIndicator status={status} lastSeen={agent.last_seen} showLabel={false} />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium text-slate-200">
                    {agent.hostname}
                  </p>
                  <p className="truncate text-[10px] text-slate-500">
                    {agent.id.slice(0, 16)}… &middot; {agent.os}
                  </p>
                </div>
                <ChevronRight
                  className={[
                    'h-4 w-4 shrink-0 transition',
                    isSelected
                      ? 'text-violet-400'
                      : 'text-slate-600 group-hover:text-slate-400',
                  ].join(' ')}
                />
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
