import React, { useEffect, useRef } from 'react';

export function Terminal({ logs, selectedAgentId }) {
  const ref = useRef(null);

  useEffect(() => {
    if (ref.current) ref.current.scrollTop = ref.current.scrollHeight;
  }, [logs]);

  const displayLogs = selectedAgentId
    ? logs.filter(l => l.agent_id === selectedAgentId || l.source.toLowerCase() === 'server')
    : logs;

  function ts(isoStr) {
    if (!isoStr) return '        ';
    try { return isoStr.split('T')[1]?.substring(0, 8) ?? isoStr; }
    catch { return isoStr; }
  }

  return (
    <div className="terminal-section">
      <div className="terminal-bar">
        <span className="dot-live" />
        <span>logs {selectedAgentId ? `— agent ${selectedAgentId.substring(0, 8)}` : '— all'}</span>
      </div>
      <div className="log-stream" ref={ref}>
        {displayLogs.length === 0
          ? <span className="log-empty">no log entries yet</span>
          : displayLogs.map((l, i) => (
              <div key={i} className="log-entry">
                <span className="le-time">{ts(l.timestamp)}</span>
                <span className={`le-lvl ${l.level.toLowerCase()}`}>{l.level}</span>
                <span className={`le-src ${l.source.toLowerCase()}`}>{l.source}</span>
                <span className="le-msg">
                  {l.agent_id && !selectedAgentId ? `[${l.agent_id.substring(0, 8)}] ` : ''}
                  {l.message}
                </span>
              </div>
            ))}
      </div>
    </div>
  );
}
