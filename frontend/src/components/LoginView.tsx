import React, { useState } from 'react';
import { api, setToken } from '../api/client';
import { PanelSettings, User } from '../types';
import { Lock, ShieldCheck, Zap } from 'lucide-react';

interface LoginViewProps {
  settings: PanelSettings | null;
  onLogin: (user: User) => void;
}

export const LoginView: React.FC<LoginViewProps> = ({ settings, onLogin }) => {
  const [username, setUsername] = useState('admin');
  const [password, setPassword] = useState('admin123');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);
    try {
      const res = await api<{ token: string; user: User }>('/api/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      });
      setToken(res.token);
      onLogin(res.user);
    } catch (err: any) {
      setError(err.message || 'Accesso fallito');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 20,
        background:
          'radial-gradient(circle at 50% 0%, rgba(245, 158, 11, 0.12), transparent 55%), var(--bg-app)',
      }}
    >
      <div className="card" style={{ width: '100%', maxWidth: 410, padding: 30 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 22 }}>
          <div className="brand-mark" style={{ width: 42, height: 42 }}>
            <Zap size={22} />
          </div>
          <div>
            <h1 style={{ fontSize: 20, fontWeight: 700, letterSpacing: '-0.02em' }}>
              {settings?.panel_name || 'GOLDSMP'}
            </h1>
            <p style={{ fontSize: 11.5, color: 'var(--text-secondary)', lineHeight: 1.3 }}>
              Greatest Optimized Lightweight Dedicated Server Management Panel
            </p>
          </div>
        </div>

        <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div className="field">
            <label htmlFor="login-user">Username</label>
            <input
              id="login-user"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoFocus
              required
            />
          </div>
          <div className="field">
            <label htmlFor="login-pass">Password</label>
            <input
              id="login-pass"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>

          {error && (
            <div
              style={{
                background: 'var(--danger-soft)',
                color: 'var(--danger)',
                padding: '9px 12px',
                borderRadius: 'var(--radius-sm)',
                fontSize: 12.5,
              }}
            >
              {error}
            </div>
          )}

          <button
            type="submit"
            className="btn btn-primary"
            style={{ width: '100%', padding: '10px 16px', marginTop: 4 }}
            disabled={loading}
          >
            <Lock size={15} />
            {loading ? 'Accesso in corso...' : 'Accedi al Pannello'}
          </button>

          {settings?.oidc_enabled && (
            <button
              type="button"
              className="btn btn-secondary"
              style={{ width: '100%' }}
              onClick={() => alert('Redirect OIDC configurato su: ' + settings.oidc_issuer_url)}
            >
              <ShieldCheck size={15} />
              Accedi con OIDC SSO
            </button>
          )}
        </form>

        <div
          style={{
            marginTop: 20,
            paddingTop: 14,
            borderTop: '1px solid var(--border-subtle)',
            fontSize: 12,
            color: 'var(--text-muted)',
            display: 'flex',
            justifyContent: 'space-between',
          }}
        >
          <span>Nessun limite di licenza</span>
          <span>Default: admin / admin123</span>
        </div>
      </div>
    </div>
  );
};
