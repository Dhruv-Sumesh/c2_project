import React from 'react';
import { formatRelativeTime } from '../api/c2Api';

const STATUS_STYLES = {
  Online: { color: 'text-emerald-400', bg: 'bg-emerald-400', label: 'Online' },
  Offline: { color: 'text-red-400', bg: 'bg-red-400', label: 'Offline' },
  Connecting: { color: 'text-amber-400', bg: 'bg-amber-400', label: 'Connecting' },
  Error: { color: 'text-red-500', bg: 'bg-red-500', label: 'Error' },
  Unknown: { color: 'text-slate-500', bg: 'bg-slate-500', label: 'Unknown' },
};

/**
 * Enhanced status badge with last-seen tooltip for educational C2 dashboard.
 */
export function StatusIndicator({ status = 'Unknown', lastSeen, size = 'sm', showLabel = true }) {
  const style = STATUS_STYLES[status] ?? STATUS_STYLES.Unknown;
  const dotSize = size === 'lg' ? 'h-2.5 w-2.5' : 'h-2 w-2';
  const tooltip = lastSeen
    ? `${style.label} · last seen ${formatRelativeTime(lastSeen)}`
    : style.label;

  return (
    <span
      className="inline-flex items-center gap-1.5"
      title={tooltip}
    >
      <span className={`${dotSize} shrink-0 rounded-full ${style.bg}`} />
      {showLabel && (
        <span className={`text-[10px] font-medium ${style.color}`}>
          {style.label.toLowerCase()}
        </span>
      )}
    </span>
  );
}

/** Summary counts panel for agent statuses */
export function StatusSummary({ agents = [] }) {
  const counts = agents.reduce((acc, a) => {
    const key = a.status ?? 'Unknown';
    acc[key] = (acc[key] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="flex flex-wrap gap-3 text-[11px]">
      {Object.entries(counts).map(([status, count]) => {
        const style = STATUS_STYLES[status] ?? STATUS_STYLES.Unknown;
        return (
          <span key={status} className={`flex items-center gap-1.5 ${style.color}`}>
            <span className={`h-1.5 w-1.5 rounded-full ${style.bg}`} />
            {count} {status.toLowerCase()}
          </span>
        );
      })}
    </div>
  );
}
