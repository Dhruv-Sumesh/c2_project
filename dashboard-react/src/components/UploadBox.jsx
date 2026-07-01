import React, { useState } from 'react';

const UploadBox = ({ sessionId }) => {
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');

  const uploadPayload = async (file) => {
    if (!file) {
      throw new Error('No file selected');
    }
    if (!sessionId) {
      throw new Error('Missing sessionId');
    }

    const formData = new FormData();
    formData.append('payload', file);

    const response = await fetch(`/api/sessions/${sessionId}/payload`, {
      method: 'POST',
      body: formData,
    });

    if (!response.ok) {
      const body = await response.text().catch(() => response.statusText);
      throw new Error(body || `Upload failed (${response.status})`);
    }

    return response.json();
  };

  const handleDragOver = (e) => {
    e.preventDefault();
  };

  const handleDrop = async (e) => {
    e.preventDefault();
    const file = e.dataTransfer.files?.[0];
    if (!file) return;

    setStatus('Uploading...');
    setError('');

    try {
      await uploadPayload(file);
      setStatus('Upload complete');
    } catch (err) {
      setError(err?.message || 'Upload failed');
      setStatus('');
    }
  };

  return (
    <div className="upload-box">
      <div
        className="upload-drop-zone"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        <p>Drag payload here</p>
        <p className="upload-hint">Drop a file to upload to the current session.</p>
      </div>

      {status && <p className="upload-status">{status}</p>}
      {error && <p className="upload-error">{error}</p>}
    </div>
  );
};

export default UploadBox;
