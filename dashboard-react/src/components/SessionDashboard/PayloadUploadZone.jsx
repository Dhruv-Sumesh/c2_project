import React, { useRef, useState, useCallback } from 'react';
import {
  Upload,
  FileUp,
  CheckCircle2,
  AlertCircle,
  Loader2,
} from 'lucide-react';
import { uploadPayload } from '../../api/c2Api';

const STATES = {
  IDLE: 'idle',
  DRAGGING: 'dragging',
  UPLOADING: 'uploading',
  SUCCESS: 'success',
  ERROR: 'error',
};

/**
 * Payload upload zone — POST /api/payloads/upload (C2 server stores under ./payloads/)
 */
export function PayloadUploadZone({ onUploadComplete }) {
  const fileInputRef = useRef(null);
  const [zoneState, setZoneState] = useState(STATES.IDLE);
  const [progress, setProgress] = useState(0);
  const [fileName, setFileName] = useState('');
  const [errorMessage, setErrorMessage] = useState('');

  const resetToIdle = useCallback(() => {
    setZoneState(STATES.IDLE);
    setProgress(0);
    setFileName('');
    setErrorMessage('');
  }, []);

  const processFile = useCallback(
    async (file) => {
      if (!file) return;

      if (file.size === 0) {
        setZoneState(STATES.ERROR);
        setErrorMessage('File is empty.');
        setTimeout(resetToIdle, 3000);
        return;
      }

      setFileName(file.name);
      setZoneState(STATES.UPLOADING);
      setProgress(0);

      try {
        const result = await uploadPayload(file, (pct) => setProgress(pct));
        setProgress(100);
        setZoneState(STATES.SUCCESS);

        if (result?.session) {
          onUploadComplete?.(result.session);
        }

        setTimeout(resetToIdle, 2500);
      } catch (err) {
        setZoneState(STATES.ERROR);
        setErrorMessage(err.message || 'Upload failed');
        setTimeout(resetToIdle, 4000);
      }
    },
    [onUploadComplete, resetToIdle],
  );

  const handleDragEnter = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (zoneState === STATES.UPLOADING) return;
    setZoneState(STATES.DRAGGING);
  };

  const handleDragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (zoneState === STATES.DRAGGING) setZoneState(STATES.IDLE);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (zoneState === STATES.UPLOADING) return;
    const file = e.dataTransfer.files?.[0];
    processFile(file);
  };

  const handleBrowse = () => fileInputRef.current?.click();

  const handleFileChange = (e) => {
    const file = e.target.files?.[0];
    processFile(file);
    e.target.value = '';
  };

  const borderClass = {
    [STATES.IDLE]: 'border-slate-600 hover:border-slate-500',
    [STATES.DRAGGING]: 'border-emerald-400 bg-emerald-500/5 ring-2 ring-emerald-400/30',
    [STATES.UPLOADING]: 'border-blue-500/60 bg-blue-500/5',
    [STATES.SUCCESS]: 'border-emerald-500/60 bg-emerald-500/5',
    [STATES.ERROR]: 'border-red-500/60 bg-red-500/5',
  }[zoneState];

  return (
    <div className="rounded-xl border border-slate-700/80 bg-slate-900/60 p-5 shadow-lg shadow-black/20">
      <div className="mb-4 flex items-center gap-2">
        <Upload className="h-4 w-4 text-emerald-400" />
        <h2 className="text-sm font-semibold tracking-wide text-slate-100">
          Payload Upload Session
        </h2>
      </div>

      <div
        role="button"
        tabIndex={0}
        aria-label="Drop payload files here"
        onDragEnter={handleDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onKeyDown={(e) => e.key === 'Enter' && handleBrowse()}
        className={[
          'relative flex min-h-[180px] cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed px-6 py-8 transition-all duration-200',
          borderClass,
        ].join(' ')}
        onClick={zoneState === STATES.IDLE ? handleBrowse : undefined}
      >
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          onChange={handleFileChange}
          accept="*/*"
        />

        {zoneState === STATES.IDLE && (
          <>
            <FileUp className="mb-3 h-10 w-10 text-slate-500" />
            <p className="text-center text-sm text-slate-300">
              Drag & drop payload files here
            </p>
            <p className="mt-1 text-center text-xs text-slate-500">
              Uploaded to C2 server via POST /api/payloads/upload
            </p>
          </>
        )}

        {zoneState === STATES.DRAGGING && (
          <>
            <FileUp className="mb-3 h-10 w-10 text-emerald-400" />
            <p className="text-sm font-medium text-emerald-300">Release to upload</p>
          </>
        )}

        {zoneState === STATES.UPLOADING && (
          <div className="w-full max-w-xs space-y-3">
            <div className="flex items-center justify-center gap-2 text-blue-300">
              <Loader2 className="h-4 w-4 animate-spin" />
              <span className="text-sm">Uploading {fileName}…</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-slate-800">
              <div
                className="h-full rounded-full bg-blue-500 transition-all duration-150"
                style={{ width: `${progress}%` }}
              />
            </div>
            <p className="text-center text-xs text-slate-500">{progress}%</p>
          </div>
        )}

        {zoneState === STATES.SUCCESS && (
          <>
            <CheckCircle2 className="mb-3 h-10 w-10 text-emerald-400" />
            <p className="text-sm font-medium text-emerald-300">Upload complete</p>
            <p className="mt-1 text-xs text-slate-400">{fileName}</p>
          </>
        )}

        {zoneState === STATES.ERROR && (
          <>
            <AlertCircle className="mb-3 h-10 w-10 text-red-400" />
            <p className="text-sm font-medium text-red-300">Upload failed</p>
            <p className="mt-1 text-xs text-red-400/80">{errorMessage}</p>
          </>
        )}
      </div>

      <div className="mt-4 flex justify-center">
        <button
          type="button"
          onClick={handleBrowse}
          disabled={zoneState === STATES.UPLOADING}
          className="inline-flex items-center gap-2 rounded-md border border-slate-600 bg-slate-800 px-4 py-2 text-xs font-medium text-slate-200 transition hover:border-slate-500 hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <Upload className="h-3.5 w-3.5" />
          Browse Files
        </button>
      </div>
    </div>
  );
}
