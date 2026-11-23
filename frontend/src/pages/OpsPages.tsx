import React, { useEffect, useState } from 'react';
import { api } from '../api/client';
import {
  AuditLogEntry,
  HostSystemInfo,
  Instance,
  PanelSettings,
  UpdateStatus,
  UpdaterConfig,
  User,
  WebhookConfig,
} from '../types';
import { DownloadCloud, Plus, RefreshCw, Save, Send, Trash2 } from 'lucide-react';
import { formatRamValue, useRamUnit } from '../utils/ramUnit';

// --- WEBHOOKS PAGE ---
export const WebhooksPage: React.FC<{
  settings: PanelSettings | null;
  onNotify: (m: string) => void;
}> = ({ settings, onNotify }) => {
  const [hooks, setHooks] = useState<WebhookConfig[]>([]);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [events, setEvents] = useState('server_start,server_stop,server_crash,player_join');

  const load = async () => setHooks(await api<WebhookConfig[]>('/api/webhooks'));
  useEffect(() => {
    load();
  }, []);

  const incomingUrl = `${window.location.origin}/api/webhooks/incoming/${
    settings?.incoming_webhook_token || '<token>'
  }`;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
      <div className="page-header" style={{ marginBottom: 4 }}>
        <div>
          <h1>Webhook & Integrazioni</h1>
          <p>Notifiche Discord/HTTP in uscita e controllo remoto via Incoming Webhook (incluso senza licenza).</p>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <span className="card-title">Webhook in Uscita (Discord / Custom HTTP)</span>
        </div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 14, flexWrap: 'wrap' }}>
          <input placeholder="Nome (es. Discord Staff)" value={name} onChange={(e) => setName(e.target.value)} />
          <input
            style={{ flex: 2, minWidth: 240 }}
            placeholder="https://discord.com/api/webhooks/..."
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <input
            style={{ flex: 1, minWidth: 200 }}
            placeholder="Eventi (* o csv)"
            value={events}
            onChange={(e) => setEvents(e.target.value)}
          />
          <button
            className="btn btn-primary btn-sm"
            onClick={async () => {
              if (!name || !url) return;
              await api('/api/webhooks', {
                method: 'POST',
                body: JSON.stringify({
                  name,
                  url,
                  events: events.split(',').map((s) => s.trim()).filter(Boolean),
                }),
              });
              setName('');
              setUrl('');
              onNotify('Webhook aggiunto');
              load();
            }}
          >
            <Plus size={14} /> Aggiungi
          </button>
        </div>

        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Nome</th>
                <th>URL</th>
                <th>Eventi</th>
                <th>Ultimo Stato</th>
                <th style={{ textAlign: 'right' }}>Azioni</th>
              </tr>
            </thead>
            <tbody>
              {hooks.map((h) => (
                <tr key={h.id}>
                  <td style={{ fontWeight: 600 }}>{h.name}</td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{h.url.slice(0, 48)}...</td>
                  <td style={{ fontSize: 12 }}>{h.events.join(', ')}</td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                    {h.last_status ?? '--'}
                  </td>
                  <td style={{ textAlign: 'right', display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={async () => {
                        await api(`/api/webhooks/${h.id}/test`, { method: 'POST' });
                        onNotify('Ping di test inviato');
                        setTimeout(load, 800);
                      }}
                    >
                      <Send size={13} /> Test
                    </button>
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={async () => {
                        await api(`/api/webhooks/${h.id}`, { method: 'DELETE' });
                        load();
                      }}
                    >
                      <Trash2 size={13} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <span className="card-title">Incoming Webhook API (Stream Deck / Automazioni esterne)</span>
        </div>
        <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 10 }}>
          Invia una richiesta POST per avviare, fermare, riavviare o eseguire comandi su qualsiasi istanza:
        </p>
        <pre
          style={{
            background: 'var(--bg-input)',
            padding: 14,
            borderRadius: 'var(--radius-sm)',
            fontFamily: 'var(--font-mono)',
            fontSize: 12,
            overflowX: 'auto',
            color: 'var(--accent)',
          }}
        >
{`curl -X POST "${incomingUrl}" \\
  -H "Content-Type: application/json" \\
  -d '{"instance_id":"<ID>","action":"command","payload":"say Riavvio tra 1 minuto!"}'`}
        </pre>
      </div>
    </div>
  );
};

// --- USERS & RBAC PAGE ---
export const UsersPage: React.FC<{ onNotify: (m: string) => void }> = ({ onNotify }) => {
  const [users, setUsers] = useState<User[]>([]);
  const [instances, setInstances] = useState<Instance[]>([]);
  const [username, setUsername] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState('operator');
  const [scopeTarget, setScopeTarget] = useState<string>('*');
  const [addInstPick, setAddInstPick] = useState<Record<string, string>>({});

  const load = async () => {
    const [u, insts] = await Promise.all([
      api<User[]>('/api/users'),
      api<Instance[]>('/api/instances'),
    ]);
    setUsers(u);
    setInstances(insts);
  };
  useEffect(() => {
    load();
  }, []);

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Utenti, Permessi Istanza (+) & Notifica Email</h1>
          <p>
            Ogni nuovo utente riceve una mail con <strong>&quot;Il tuo account per GOLDSMP è attivo&quot;</strong> e può
            essere limitato a una singola istanza o abilitato su altre istanze con il pulsante <strong>+</strong>.
          </p>
        </div>
      </div>

      <div className="card" style={{ marginBottom: 18 }}>
        <div className="card-header">
          <span className="card-title">Nuovo Utente (Invia Email &quot;Il tuo account per GOLDSMP è attivo&quot;)</span>
        </div>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <input placeholder="Username" value={username} onChange={(e) => setUsername(e.target.value)} />
          <input
            placeholder="Email proprietario"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            style={{ minWidth: 210 }}
          />
          <input
            type="password"
            placeholder="Password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <select value={role} onChange={(e) => setRole(e.target.value)}>
            <option value="superadmin">SuperAdmin</option>
            <option value="admin">Admin</option>
            <option value="operator">Operator</option>
            <option value="viewer">Viewer (Solo Lettura)</option>
          </select>
          <select value={scopeTarget} onChange={(e) => setScopeTarget(e.target.value)}>
            <option value="*">Accesso: Intero Server (Tutte le istanze)</option>
            {instances.map((inst) => (
              <option key={inst.id} value={inst.id}>
                Solo Istanza: {inst.name}
              </option>
            ))}
          </select>
          <button
            className="btn btn-primary btn-sm"
            onClick={async () => {
              if (!username || !password) return;
              try {
                const permissions =
                  scopeTarget === '*'
                    ? ['*']
                    : [`instance:${scopeTarget}`, 'instances.read', 'instances.control', 'files.write'];
                await api('/api/users', {
                  method: 'POST',
                  body: JSON.stringify({ username, email, password, role, permissions }),
                });
                setUsername('');
                setEmail('');
                setPassword('');
                onNotify('Utente creato e mail "Il tuo account per GOLDSMP è attivo" inviata!');
                load();
              } catch (err: any) {
                onNotify(err.message);
              }
            }}
          >
            <Plus size={14} /> Crea Utente & Invia Mail
          </button>
        </div>
      </div>

      <div className="card">
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Username</th>
                <th>Email</th>
                <th>Ruolo</th>
                <th>Istanze Abilitate</th>
                <th>Aggiungi Permesso Istanza (+)</th>
                <th style={{ textAlign: 'right' }}>Azioni</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => {
                const isGlobal =
                  u.role === 'superadmin' || u.role === 'admin' || u.permissions.includes('*');
                const allowedIds = u.permissions
                  .filter((p) => p.startsWith('instance:'))
                  .map((p) => p.replace('instance:', ''));
                const unassigned = instances.filter((i) => !allowedIds.includes(i.id));

                return (
                  <tr key={u.id}>
                    <td style={{ fontWeight: 600 }}>{u.username}</td>
                    <td>{u.email}</td>
                    <td>
                      <span className="status-badge status-running">{u.role}</span>
                    </td>
                    <td>
                      {isGlobal ? (
                        <span className="status-badge status-running">Intero Server (*)</span>
                      ) : allowedIds.length === 0 ? (
                        <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Nessuna</span>
                      ) : (
                        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                          {allowedIds.map((iid) => {
                            const instName =
                              instances.find((x) => x.id === iid)?.name || iid.slice(0, 8);
                            return (
                              <span
                                key={iid}
                                className="status-badge status-starting"
                                style={{ textTransform: 'none' }}
                              >
                                {instName}
                              </span>
                            );
                          })}
                        </div>
                      )}
                    </td>
                    <td>
                      {!isGlobal && unassigned.length > 0 ? (
                        <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                          <select
                            style={{ padding: '4px 8px', fontSize: 12 }}
                            value={addInstPick[u.id] || unassigned[0].id}
                            onChange={(e) =>
                              setAddInstPick((prev) => ({ ...prev, [u.id]: e.target.value }))
                            }
                          >
                            {unassigned.map((inst) => (
                              <option key={inst.id} value={inst.id}>
                                {inst.name}
                              </option>
                            ))}
                          </select>
                          <button
                            className="btn btn-primary btn-sm"
                            title="Aggiungi permesso per questa istanza"
                            onClick={async () => {
                              const targetId = addInstPick[u.id] || unassigned[0].id;
                              await api(`/api/users/${u.id}/instances`, {
                                method: 'POST',
                                body: JSON.stringify({ instance_id: targetId }),
                              });
                              onNotify(`Permesso aggiunto (+) a ${u.username}`);
                              load();
                            }}
                          >
                            <Plus size={13} />
                          </button>
                        </div>
                      ) : (
                        <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Tutte abilitate</span>
                      )}
                    </td>
                    <td style={{ textAlign: 'right' }}>
                      {u.username !== 'admin' && (
                        <button
                          className="btn btn-danger btn-sm"
                          onClick={async () => {
                            await api(`/api/users/${u.id}`, { method: 'DELETE' });
                            load();
                          }}
                        >
                          <Trash2 size={13} />
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

// --- AUDIT LOG PAGE ---
export const AuditLogPage: React.FC = () => {
  const [logs, setLogs] = useState<AuditLogEntry[]>([]);
  useEffect(() => {
    api<AuditLogEntry[]>('/api/audit-logs').then(setLogs);
  }, []);

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Audit Log Completo</h1>
          <p>Tracciamento dettagliato di ogni azione amministrativa, comando console, modifica file e login.</p>
        </div>
      </div>

      <div className="card">
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Timestamp</th>
                <th>Utente</th>
                <th>Azione</th>
                <th>Target</th>
                <th>Dettagli</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((l) => (
                <tr key={l.id}>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text-secondary)' }}>
                    {new Date(l.timestamp).toLocaleString()}
                  </td>
                  <td style={{ fontWeight: 600 }}>{l.username}</td>
                  <td>
                    <span className="status-badge status-stopped">{l.action}</span>
                  </td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{l.target}</td>
                  <td style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>{l.details}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

// --- SETTINGS & BRANDING PAGE ---
export const SettingsPage: React.FC<{
  settings: PanelSettings | null;
  sysInfo: HostSystemInfo | null;
  onSaved: (s: PanelSettings) => void;
  onNotify: (m: string) => void;
}> = ({ settings, sysInfo, onSaved, onNotify }) => {
  const [form, setForm] = useState<PanelSettings | null>(settings);
  const [updStatus, setUpdStatus] = useState<UpdateStatus | null>(null);
  const [updCfg, setUpdCfg] = useState<UpdaterConfig | null>(null);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (settings) setForm(settings);
  }, [settings]);

  useEffect(() => {
    api<UpdateStatus>('/api/updater/status')
      .then((s) => {
        setUpdStatus(s);
        setUpdCfg(s.config);
      })
      .catch(() => {});
  }, []);

  if (!form) return null;

  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    const updated = await api<PanelSettings>('/api/settings', {
      method: 'PUT',
      body: JSON.stringify(form),
    });
    if (updCfg) {
      const u = await api<UpdateStatus>('/api/updater/config', {
        method: 'PUT',
        body: JSON.stringify(updCfg),
      });
      setUpdStatus(u);
    }
    onSaved(updated);
    onNotify('Impostazioni e configurazione aggiornamenti salvate');
  };

  const [ramUnit] = useRamUnit();
  const [restartingBin, setRestartingBin] = useState(false);

  const checkUpdates = async (forceApply = false) => {
    setChecking(true);
    try {
      const st = await api<UpdateStatus>(
        forceApply ? '/api/updater/apply' : '/api/updater/check',
        { method: 'POST' }
      );
      setUpdStatus(st);
      if (st.state === 'ready_refresh') {
        onNotify(
          `Aggiornamento v${st.latest_version || st.active_ui_version} applicato! Premi Aggiorna Pagina (F5) o Riavvia Binario.`
        );
      } else if (st.update_available) {
        onNotify(`Trovata nuova versione v${st.latest_version} su GitHub! Clicca Scarica & Attiva.`);
      } else if (st.state === 'up_to_date') {
        onNotify(`GOLDSMP è già aggiornato (v${st.active_ui_version})`);
      } else if (st.error) {
        onNotify(st.error);
      }
    } finally {
      setChecking(false);
    }
  };

  const restartPanelBinary = async () => {
    setRestartingBin(true);
    try {
      await api('/api/updater/restart', { method: 'POST' });
      onNotify('Riavvio binario GOLDSMP in corso... ricarico pagina tra 2 secondi');
      setTimeout(() => window.location.reload(), 2000);
    } catch (err: any) {
      onNotify(err.message || 'Errore riavvio binario');
      setRestartingBin(false);
    }
  };

  return (
    <form onSubmit={save} style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
      <div className="page-header" style={{ marginBottom: 4 }}>
        <div>
          <h1>Impostazioni, Auto-Updater & OIDC SSO</h1>
          <p>Aggiornamenti automatici senza interruzioni, branding white-label e autenticazione OIDC.</p>
        </div>
        <button type="submit" className="btn btn-primary">
          <Save size={15} /> Salva Tutto
        </button>
      </div>

      {/* Zero-Downtime Auto-Updater Card */}
      {updStatus && updCfg && (
        <div className="card">
          <div className="card-header" style={{ flexWrap: 'wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <DownloadCloud size={18} color="var(--accent)" />
              <span className="card-title">Aggiornamenti Automatici (Zero-Downtime)</span>
              <span
                className={`status-badge ${
                  updStatus.state === 'ready_refresh'
                    ? 'status-starting'
                    : updStatus.state === 'error'
                    ? 'status-crashed'
                    : 'status-running'
                }`}
              >
                <span className="status-dot" />
                {updStatus.state === 'ready_refresh'
                  ? 'Pronto — Premi F5'
                  : updStatus.state === 'up_to_date'
                  ? 'Aggiornato'
                  : updStatus.state}
              </span>
            </div>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                disabled={checking}
                onClick={() => checkUpdates(false)}
              >
                <RefreshCw size={13} /> {checking ? 'Controllo...' : 'Controlla Ora'}
              </button>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                disabled={checking}
                onClick={() => checkUpdates(true)}
                title="Scarica e installa forzatamente l'ultima versione da GitHub (Release o Git)"
              >
                <DownloadCloud size={13} />{' '}
                {updStatus.update_available
                  ? `Scarica & Attiva v${updStatus.latest_version}`
                  : 'Forza Aggiornamento da GitHub'}
              </button>
              {(updStatus.ui_hot_updated || updStatus.state === 'ready_refresh') && (
                <button
                  type="button"
                  className="btn btn-success btn-sm"
                  onClick={() => window.location.reload()}
                >
                  <RefreshCw size={13} /> Aggiorna Pagina (F5)
                </button>
              )}
              {updStatus.binary_staged && (
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  disabled={restartingBin}
                  onClick={restartPanelBinary}
                  title="Sostituisce il processo del pannello con il nuovo binario compilato"
                >
                  <RefreshCw size={13} />{' '}
                  {restartingBin ? 'Riavvio...' : 'Riavvia Binario Pannello'}
                </button>
              )}
            </div>
          </div>

          <p style={{ fontSize: 12.5, color: 'var(--text-secondary)', marginBottom: 14 }}>
            GOLDSMP controlla automaticamente le release GitHub, scarica il nuovo bundle UI a caldo e prepara il binario su disco in modo atomico: i server di gioco in esecuzione non vengono mai interrotti e basta premere <strong>Refresh (F5)</strong> nel browser.
          </p>

          <div className="grid-4" style={{ marginBottom: 14, fontFamily: 'var(--font-mono)', fontSize: 12.5 }}>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>Versione Binario</div>
              <div>v{updStatus.current_version}</div>
            </div>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>Versione UI Attiva</div>
              <div style={{ color: 'var(--accent)' }}>v{updStatus.active_ui_version}</div>
            </div>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>Ultima su GitHub</div>
              <div>{updStatus.latest_version ? `v${updStatus.latest_version}` : '--'}</div>
            </div>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>Ultimo Controllo</div>
              <div>
                {updStatus.last_checked
                  ? new Date(updStatus.last_checked).toLocaleTimeString()
                  : 'In attesa'}
              </div>
            </div>
          </div>

          <div className="grid-3">
            <div className="field">
              <label>Repository GitHub (owner/repo)</label>
              <input
                value={updCfg.repo}
                onChange={(e) => setUpdCfg({ ...updCfg, repo: e.target.value })}
              />
            </div>
            <div className="field">
              <label>Intervallo Controllo (minuti)</label>
              <input
                type="number"
                min={5}
                value={updCfg.check_interval_mins}
                onChange={(e) =>
                  setUpdCfg({ ...updCfg, check_interval_mins: Number(e.target.value) })
                }
              />
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: 8 }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                <input
                  type="checkbox"
                  checked={updCfg.auto_check}
                  onChange={(e) => setUpdCfg({ ...updCfg, auto_check: e.target.checked })}
                />
                Controllo automatico in background
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                <input
                  type="checkbox"
                  checked={updCfg.auto_download}
                  onChange={(e) => setUpdCfg({ ...updCfg, auto_download: e.target.checked })}
                />
                Scarica e applica UI automaticamente (solo F5)
              </label>
            </div>
          </div>
          {updStatus.error && (
            <div style={{ marginTop: 10, color: 'var(--danger)', fontSize: 12.5 }}>
              {updStatus.error}
            </div>
          )}
        </div>
      )}

      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <span className="card-title">Branding Personalizzato</span>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div className="field">
              <label>Nome Pannello</label>
              <input
                value={form.panel_name}
                onChange={(e) => setForm({ ...form, panel_name: e.target.value })}
              />
            </div>
            <div className="field">
              <label>Colore Accento Principale</label>
              <div style={{ display: 'flex', gap: 8 }}>
                <input
                  type="color"
                  style={{ width: 52, padding: 2 }}
                  value={form.accent_color}
                  onChange={(e) => setForm({ ...form, accent_color: e.target.value })}
                />
                <input
                  value={form.accent_color}
                  onChange={(e) => setForm({ ...form, accent_color: e.target.value })}
                />
              </div>
            </div>
            <div className="field">
              <label>Retention Metriche SQLite (Giorni)</label>
              <input
                type="number"
                value={form.metrics_retention_days}
                onChange={(e) =>
                  setForm({ ...form, metrics_retention_days: Number(e.target.value) })
                }
              />
            </div>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">OIDC Single Sign-On (Authentik / Keycloak / Authelia)</span>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
              <input
                type="checkbox"
                checked={form.oidc_enabled}
                onChange={(e) => setForm({ ...form, oidc_enabled: e.target.checked })}
              />
              Abilita Login OIDC SSO
            </label>
            <div className="field">
              <label>Issuer URL</label>
              <input
                placeholder="https://auth.example.com/application/o/goldsmp/"
                value={form.oidc_issuer_url}
                onChange={(e) => setForm({ ...form, oidc_issuer_url: e.target.value })}
              />
            </div>
            <div className="field">
              <label>Client ID</label>
              <input
                value={form.oidc_client_id}
                onChange={(e) => setForm({ ...form, oidc_client_id: e.target.value })}
              />
            </div>
            <div className="field">
              <label>Client Secret</label>
              <input
                type="password"
                value={form.oidc_client_secret}
                onChange={(e) => setForm({ ...form, oidc_client_secret: e.target.value })}
              />
            </div>
          </div>
        </div>
      </div>

      {sysInfo && (
        <div className="card">
          <div className="card-header">
            <span className="card-title">Diagnostica Sistema & Impronta Rust</span>
          </div>
          <div className="grid-4" style={{ fontFamily: 'var(--font-mono)', fontSize: 12.5 }}>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>Hostname</div>
              <div>{sysInfo.hostname}</div>
            </div>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>OS / Kernel</div>
              <div>
                {sysInfo.os_name} {sysInfo.os_version} ({sysInfo.kernel_version})
              </div>
            </div>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>CPU</div>
              <div>
                {sysInfo.cpu_brand} ({sysInfo.cpu_cores} core)
              </div>
            </div>
            <div>
              <div style={{ color: 'var(--text-muted)' }}>RAM Processo GOLDSMP</div>
              <div style={{ color: 'var(--success)', fontWeight: 700 }}>
                {formatRamValue(sysInfo.panel_memory_mb, ramUnit, ramUnit === 'GB' ? 3 : 2)} (v{sysInfo.panel_version})
              </div>
            </div>
          </div>
        </div>
      )}
    </form>
  );
};
