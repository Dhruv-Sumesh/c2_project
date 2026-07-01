import React from 'react';
import { Clock, FileText } from 'lucide-react';

const STATUS_STYLES = {
  Pending: 'bg-amber-500/15 text-amber-300 ring-amber-500/30',
  Active: 'bg-emerald-500/15 text-emerald-300 ring-emerald-500/30',
  Completed: 'bg-slate-500/15 text-slate-300 ring-slate-500/30',
};

function formatSize(bytes) {
  if (!bytes) return '—';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTime(iso) {
  try {
    return new Date(iso).toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return iso;
  }
}

/** Lists active and past payload upload sessions with status badges */
export function UploadSessionList({ sessions }) {
  return (
    <div className="rounded-xl border border-slate-700/80 bg-slate-900/60 p-5 shadow-lg shadow-black/20">
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <FileText className="h-4 w-4 text-blue-400" />
          <h3 className="text-sm font-semibold text-slate-100">Upload Sessions</h3>
        </div>
        <span className="rounded-full bg-slate-800 px-2 py-0.5 text-[10px] text-slate-400">
          {sessions.length} total
        </span>
      </div>

      {sessions.length === 0 ? (
        <p className="py-6 text-center text-xs text-slate-500">
          No upload sessions yet. Drop a payload above to start.
        </p>
      ) : (
        <ul className="space-y-2">
          {sessions.map((session) => (
            <li
              key={session.id}
              className="flex items-center justify-between gap-3 rounded-lg border border-slate-700/60 bg-slate-800/40 px-3 py-2.5 transition hover:bg-slate-800/70"
            >
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm text-slate-200">{session.fileName}</p>
                <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[10px] text-slate-500">
                  <span>{formatSize(session.fileSize)}</span>
                  <span className="inline-flex items-center gap-1">
                    <Clock className="h-3 w-3" />
                    {formatTime(session.uploadedAt)}
                  </span>
                </div>
              </div>
              <span
                className={[
                  'shrink-0 rounded-full px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-wide ring-1 ring-inset',
                  STATUS_STYLES[session.status] ?? STATUS_STYLES.Pending,
                ].join(' ')}
              >
                {session.status}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
