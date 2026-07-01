import React, { useState } from 'react';

// Default operator credentials (frontend-only, change as needed)
const OPERATOR_USER = 'operator';
const OPERATOR_PASS = 'c2admin';

export function Login({ onLogin }) {
  const [user, setUser] = useState('');
  const [pass, setPass] = useState('');
  const [error, setError] = useState('');

  function handleSubmit(e) {
    e.preventDefault();
    setError('');
    if (user === OPERATOR_USER && pass === OPERATOR_PASS) {
      onLogin();
    } else {
      setError('invalid credentials');
    }
  }

  return (
    <div className="login-page">
      <div className="login-box">
        <div className="login-title">c2-simulator</div>
        <div className="login-sub">operator access</div>

        <form onSubmit={handleSubmit} className="login-form">
          <div className="field">
            <label htmlFor="login-user">username</label>
            <input
              id="login-user"
              type="text"
              value={user}
              autoComplete="off"
              spellCheck="false"
              autoFocus
              onChange={e => setUser(e.target.value)}
            />
          </div>
          <div className="field">
            <label htmlFor="login-pass">password</label>
            <input
              id="login-pass"
              type="password"
              value={pass}
              onChange={e => setPass(e.target.value)}
            />
          </div>
          {error && <div className="login-error">{error}</div>}
          <button type="submit" className="login-btn">connect</button>
        </form>

        <div className="login-hint">default: operator / c2admin</div>
      </div>
    </div>
  );
}
