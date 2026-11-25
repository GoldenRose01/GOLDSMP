import React, { useEffect, useState } from 'react';
import { api } from '../api/client';
import {
  AuditLogEntry,
  HostSystemInfo,
  Instance,
  PanelSettings,
  SentinelConfig,
  SentinelStatus,
  UpdateStatus,
  UpdaterConfig,
  User,
  WebhookConfig,
} from '../types';
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  DownloadCloud,
  Plus,
  RefreshCw,
  Save,
  Send,
  ShieldCheck,
  Trash2,
} from 'lucide-react';
import { formatRamValue, useRamUnit } from '../utils/ramUnit';

// --- COOLIFY SENTINEL WATCHDOG & HEALTH AGENT PAGE ---
export const SentinelPage: React.FC<{
  onSelectInstance: (id: string) => void;
  onNotify: (m: string) => void;
}> = ({ onSelectInstance, onNotify }) => {
  const [status, setStatus] = useState<SentinelStatus | null>(null);
  const [cfg, setCfg] = useState<SentinelConfig | null>(null);
  const [runningCheck, setRunningCheck] = useState(false);
  const [saving, setSaving] = useState(false);
  const [ramUnit] = useRamUnit();

  const load = async () => {
    try {
      const s = await api<SentinelStatus>('/api/sentinel/status');
      setStatus(s);
      if (!cfg) setCfg(s.config);
    } catch {}
  };

  useEffect(() => {
    load();
    const t = setInterval(load, 4000);
    return () => clearInterval(t);
  }, []);

  const triggerCheck = async () => {
    setRunningCheck(true);
    try {
      const s = await api<SentinelStatus>('/api/sentinel/check', { method: 'POST' });
      setStatus(s);
      onNotify(
        `Heartbeat Coolify Sentinel completato: ${s.instances.length} istanze controllate (${s.agent_state})`
      );
    } catch (err: any) {
      onNotify(err.message);
    } finally {
      setRunningCheck(false);
    }
  };

  const saveSentinelConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!cfg) return;
    setSaving(true);
    try {
      const s = await api<SentinelStatus>('/api/sentinel/config', {
        method: 'PUT',
        body: JSON.stringify(cfg),
      });
      setStatus(s);
      setCfg(s.config);
      onNotify('Configurazione Coolify Sentinel salvata!');
    } catch (err: any) {
      onNotify(err.message);
    } finally {
      setSaving(false);
    }
  };

  if (!status || !cfg) {
    return <div className="card">Inizializzazione Coolify Sentinel...</div>;
  }

  const healthyCount = status.instances.filter((i) => i.health_state === 'healthy').length;
  const activeCount = status.instances.filter((i) => i.process_status === 'running').length;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
      <div className="page-header" style={{ marginBottom: 4 }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
            <h1>Coolify Sentinel • Watchdog & Auto-Healing</h1>
            <span
              className={`status-badge ${
                status.agent_state === 'active'
                  ? 'status-running'
                  : status.agent_state === 'warning'
                  ? 'status-starting'
                  : status.agent_state === 'critical'
                  ? 'status-crashed'
                  : 'status-stopped'
              }`}
            >
              <span className="status-dot" />
              Sentinel {status.agent_state}
            </span>
          </div>
          <p>
            Controllo continuo liveness TCP sulle porte, soglie risorse Host/Istanze, auto-heal dei processi crashati e push telemetria compatibile con Coolify Sentinel.
          </p>
        </div>

        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <button
            id="btn-sentinel-force-check"
            type="button"
            className="btn btn-primary"
            disabled={runningCheck}
            onClick={triggerCheck}
          >
            <RefreshCw size={15} />
            {runningCheck ? 'Sonda TCP in corso...' : 'Esegui Check Sentinel Ora'}
          </button>
        </div>
      </div>

      {/* Sentinel KPIs */}
      <div className="grid-4">
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--success)' }}>
            {healthyCount} / {activeCount}
          </div>
          <div className="kpi-label">Istanze Attive Healthy (TCP OK)</div>
        </div>
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--accent)' }}>
            {status.total_checks}
          </div>
          <div className="kpi-label">
            Cicli Heartbeat ({status.last_heartbeat ? new Date(status.last_heartbeat).toLocaleTimeString() : '--'})
          </div>
        </div>
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--arc-cyan)' }}>
            {status.total_auto_heals}
          </div>
          <div className="kpi-label">Auto-Heal Eseguiti (Self-Repair)</div>
        </div>
        <div className="card">
          <div
            className="kpi-value"
            style={{
              color:
                status.active_warnings.length > 0 ? 'var(--warning)' : 'var(--success)',
            }}
          >
            {status.active_warnings.length}
          </div>
          <div className="kpi-label">Allerte Soglia Attive</div>
        </div>
      </div>

      {/* Active Warnings Banner if any */}
      {status.active_warnings.length > 0 && (
        <div
          className="card"
          style={{
            background: 'var(--warning-soft)',
            borderColor: 'rgba(245, 185, 50, 0.35)',
          }}
        >
          <div className="card-header" style={{ marginBottom: 8 }}>
            <span className="card-title" style={{ color: 'var(--accent-hover)' }}>
              <AlertTriangle size={16} /> Allerte Rilevate da Coolify Sentinel
            </span>
          </div>
          <ul style={{ paddingLeft: 18, fontSize: 13, display: 'flex', flexDirection: 'column', gap: 4 }}>
            {status.active_warnings.map((w, idx) => (
              <li key={idx}>{w}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Per-Instance Sentinel Health Matrix */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <Activity size={16} color="var(--arc-cyan)" /> Matrice Liveness Istanze & Probe TCP Porte
          </span>
          <span style={{ fontSize: 12, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
            Intervallo sonda: ogni {cfg.check_interval_secs}s
          </span>
        </div>

        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Istanza</th>
                <th>Porta TCP</th>
                <th>Stato Processo</th>
                <th>Salute Sentinel</th>
                <th>Latenza TCP</th>
                <th>CPU / RAM ({ramUnit})</th>
                <th>Auto-Heal</th>
              </tr>
            </thead>
            <tbody>
              {status.instances.map((inst) => (
                <tr
                  key={inst.instance_id}
                  style={{ cursor: 'pointer' }}
                  onClick={() => onSelectInstance(inst.instance_id)}
                >
                  <td style={{ fontWeight: 600 }}>{inst.instance_name}</td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>:{inst.port}</td>
                  <td>
                    <span className={`status-badge status-${inst.process_status}`}>
                      <span className="status-dot" />
                      {inst.process_status}
                    </span>
                  </td>
                  <td>
                    <span
                      className={`status-badge ${
                        inst.health_state === 'healthy'
                          ? 'status-running'
                          : inst.health_state === 'degraded' || inst.health_state === 'starting' || inst.health_state === 'healing'
                          ? 'status-starting'
                          : inst.health_state === 'unhealthy'
                          ? 'status-crashed'
                          : 'status-stopped'
                      }`}
                    >
                      {inst.health_state}
                    </span>
                  </td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                    {inst.port_reachable ? (
                      <span style={{ color: 'var(--success)' }}>{inst.latency_ms ?? 1} ms (OK)</span>
                    ) : inst.process_status === 'running' ? (
                      <span style={{ color: 'var(--warning)' }}>
                        Non raggiungibile ({inst.consecutive_failures} fail)
                      </span>
                    ) : (
                      <span style={{ color: 'var(--text-muted)' }}>--</span>
                    )}
                  </td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                    {inst.cpu_percent.toFixed(1)}% • {formatRamValue(inst.memory_mb, ramUnit)} ({inst.memory_usage_pct.toFixed(0)}%)
                  </td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                    {inst.auto_heals_count}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Sentinel Configuration & Coolify Push Endpoint + Event Log */}
      <div className="grid-2">
        <form onSubmit={saveSentinelConfig} className="card" style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <div className="card-header" style={{ marginBottom: 2 }}>
            <span className="card-title">
              <ShieldCheck size={16} color="var(--accent)" /> Configurazione Watchdog & Coolify Push
            </span>
            <button id="btn-save-sentinel-config" type="submit" className="btn btn-primary btn-sm" disabled={saving}>
              <Save size={13} /> {saving ? 'Salvataggio...' : 'Salva Sentinel'}
            </button>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
              <input
                type="checkbox"
                checked={cfg.enabled}
                onChange={(e) => setCfg({ ...cfg, enabled: e.target.checked })}
              />
              Abilita Coolify Sentinel Watchdog in background
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
              <input
                type="checkbox"
                checked={cfg.auto_heal_crashed}
                onChange={(e) => setCfg({ ...cfg, auto_heal_crashed: e.target.checked })}
              />
              Auto-Heal automatico se un&apos;istanza entra in stato Crashed
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
              <input
                type="checkbox"
                checked={cfg.auto_heal_unresponsive_port}
                onChange={(e) => setCfg({ ...cfg, auto_heal_unresponsive_port: e.target.checked })}
              />
              Auto-Heal se la porta TCP non risponde per 3 cicli consecutivi
            </label>
          </div>

          <div className="grid-2">
            <div className="field">
              <label>Intervallo Heartbeat (secondi)</label>
              <input
                type="number"
                min={5}
                max={600}
                value={cfg.check_interval_secs}
                onChange={(e) => setCfg({ ...cfg, check_interval_secs: Number(e.target.value) })}
              />
            </div>
            <div className="field">
              <label>Soglia Allerta CPU Host (%)</label>
              <input
                type="number"
                min={20}
                max={100}
                value={cfg.cpu_warn_threshold}
                onChange={(e) => setCfg({ ...cfg, cpu_warn_threshold: Number(e.target.value) })}
              />
            </div>
            <div className="field">
              <label>Soglia Allerta RAM (%)</label>
              <input
                type="number"
                min={20}
                max={100}
                value={cfg.mem_warn_threshold}
                onChange={(e) => setCfg({ ...cfg, mem_warn_threshold: Number(e.target.value) })}
              />
            </div>
            <div className="field">
              <label>Soglia Allerta Disco (%)</label>
              <input
                type="number"
                min={20}
                max={100}
                value={cfg.disk_warn_threshold}
                onChange={(e) => setCfg({ ...cfg, disk_warn_threshold: Number(e.target.value) })}
              />
            </div>
          </div>

          <div className="field">
            <label>Coolify Sentinel Push Endpoint URL (opzionale)</label>
            <input
              placeholder="https://coolify.example.com/api/v1/sentinel/push"
              value={cfg.push_endpoint}
              onChange={(e) => setCfg({ ...cfg, push_endpoint: e.target.value })}
            />
          </div>
          <div className="field">
            <label>Coolify Sentinel Bearer Token (opzionale)</label>
            <input
              type="password"
              placeholder="Token autenticazione Coolify Sentinel..."
              value={cfg.push_token}
              onChange={(e) => setCfg({ ...cfg, push_token: e.target.value })}
            />
            {status.last_push_status && (
              <span className="field-hint">Ultimo push esterno: {status.last_push_status}</span>
            )}
          </div>
        </form>

        <div className="card" style={{ display: 'flex', flexDirection: 'column' }}>
          <div className="card-header">
            <span className="card-title">
              <CheckCircle2 size={16} color="var(--success)" /> Registro Eventi Sentinel
            </span>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8, maxHeight: 390, overflowY: 'auto' }}>
            {status.recent_events.map((ev, i) => (
              <div
                key={i}
                style={{
                  padding: '9px 12px',
                  borderRadius: 'var(--radius-sm)',
                  background: 'var(--bg-input)',
                  border: '1px solid var(--border-subtle)',
                  fontSize: 12.5,
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 3,
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 8 }}>
                  <span
                    className={`status-badge ${
                      ev.level === 'critical'
                        ? 'status-crashed'
                        : ev.level === 'warn'
                        ? 'status-starting'
                        : ev.level === 'heal'
                        ? 'status-running'
                        : 'status-stopped'
                    }`}
                    style={{ fontSize: 10, padding: '1px 7px' }}
                  >
                    {ev.level} • {ev.target}
                  </span>
                  <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-muted)' }}>
                    {new Date(ev.timestamp).toLocaleTimeString()}
                  </span>
                </div>
                <div style={{ color: 'var(--text-primary)' }}>{ev.message}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};


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

  // Install-time OAuth Client (Google Drive & OIDC) state
  const [gdStatus, setGdStatus] = useState<any>(null);
  const [gdClientId, setGdClientId] = useState('');
  const [gdClientSecret, setGdClientSecret] = useState('');
  const [gdRedirectUri, setGdRedirectUri] = useState(
    `${window.location.origin}/api/gdrive/callback`
  );
  const [gdFolder, setGdFolder] = useState('GOLDSMP-Server');
  const [gdMaxBackups, setGdMaxBackups] = useState(5);
  const [gdEnabled, setGdEnabled] = useState(true);

  useEffect(() => {
    if (settings) setForm(settings);
  }, [settings]);

  const loadGdriveStatus = async () => {
    try {
      const g = await api<any>('/api/gdrive/status');
      setGdStatus(g);
      setGdClientId(g.client_id || '');
      setGdRedirectUri(g.redirect_uri || `${window.location.origin}/api/gdrive/callback`);
      setGdFolder(g.server_folder_name || 'GOLDSMP-Server');
      setGdMaxBackups(g.max_backups_per_instance || 5);
      setGdEnabled(g.enabled ?? true);
    } catch {}
  };

  useEffect(() => {
    api<UpdateStatus>('/api/updater/status')
      .then((s) => {
        setUpdStatus(s);
        setUpdCfg(s.config);
      })
      .catch(() => {});
    loadGdriveStatus();
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
    const gUpdated = await api<any>('/api/gdrive/config', {
      method: 'PUT',
      body: JSON.stringify({
        enabled: gdEnabled,
        client_id: gdClientId,
        client_secret: gdClientSecret || undefined,
        redirect_uri: gdRedirectUri,
        server_folder_name: gdFolder,
        max_backups_per_instance: Number(gdMaxBackups) || 5,
      }),
    }).catch(() => null);
    if (gUpdated) {
      setGdStatus(gUpdated);
      setGdClientSecret('');
    }
    onSaved(updated);
    onNotify('Impostazioni, Client OAuth e configurazione aggiornamenti salvati');
  };

  const handleImportOAuthJson = async (file: File | null) => {
    if (!file) return;
    try {
      const raw = await file.text();
      const updated = await api<any>('/api/gdrive/import-json', {
        method: 'POST',
        body: JSON.stringify({
          raw_json: raw,
          persist_install_file: true,
        }),
      });
      setGdStatus(updated);
      setGdClientId(updated.client_id || '');
      setGdRedirectUri(updated.redirect_uri || gdRedirectUri);
      onNotify('Client OAuth importato e salvato nelle condizioni di installazione (oauth_client.json)!');
    } catch (err: any) {
      onNotify(`Errore importazione JSON OAuth: ${err.message}`);
    }
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
    <form id="form-panel-settings" onSubmit={save} style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
      <div className="page-header" style={{ marginBottom: 4 }}>
        <div>
          <h1>Impostazioni, Client OAuth & Auto-Updater</h1>
          <p>Provisioning Client OAuth d&apos;installazione (Google Drive & OIDC SSO), aggiornamenti zero-downtime e branding.</p>
        </div>
        <button id="btn-save-all-settings" type="submit" className="btn btn-primary">
          <Save size={15} /> Salva Tutto
        </button>
      </div>

      {/* Install-Time OAuth Client Provisioning Card */}
      <div className="card">
        <div className="card-header" style={{ flexWrap: 'wrap' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
            <span className="card-title">Client OAuth (Condizioni di Installazione & Google Cloud Backup)</span>
            <span
              className={`status-badge ${
                gdStatus?.connected
                  ? 'status-running'
                  : gdClientId
                  ? 'status-starting'
                  : 'status-stopped'
              }`}
            >
              <span className="status-dot" />
              {gdStatus?.connected
                ? `Connesso (${gdStatus.connected_email || gdStatus.auth_mode || 'Attivo'})`
                : gdClientId
                ? 'Client OAuth Preconfigurato'
                : 'In attesa di Client OAuth'}
            </span>
          </div>

          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
            <label
              htmlFor="input-import-oauth-json-settings"
              className="btn btn-secondary btn-sm"
              style={{ cursor: 'pointer' }}
              title="Carica il file client_secret.json / oauth_client.json di Google Cloud e salvalo nelle condizioni di installazione"
            >
              Importa client_secret.json (Installazione)
            </label>
            <input
              id="input-import-oauth-json-settings"
              type="file"
              accept=".json,application/json"
              style={{ display: 'none' }}
              onChange={(e) => handleImportOAuthJson(e.target.files?.[0] || null)}
            />
            <button
              id="btn-open-oauth-portal-settings"
              type="button"
              className="btn btn-primary btn-sm"
              onClick={() => {
                const popup = window.open(
                  '/api/gdrive/login?portal=1',
                  'goldsmp_oauth_portal',
                  'width=700,height=780'
                );
                const t = setInterval(() => {
                  if (!popup || popup.closed) {
                    clearInterval(t);
                    loadGdriveStatus();
                  }
                }, 1200);
              }}
            >
              Apri Portale Login & Scelta Account OAuth
            </button>
          </div>
        </div>

        <p style={{ fontSize: 12.5, color: 'var(--text-secondary)', marginBottom: 14 }}>
          Il client OAuth viene caricato automaticamente all&apos;installazione da{' '}
          <code style={{ color: 'var(--accent-hover)' }}>goldsmp-data/oauth_client.json</code> o dalle variabili{' '}
          <code style={{ color: 'var(--accent-hover)' }}>GOLDSMP_OAUTH_CLIENT_ID</code> /{' '}
          <code style={{ color: 'var(--accent-hover)' }}>GOLDSMP_OAUTH_CLIENT_SECRET</code>. Puoi anche importare il JSON di Google Cloud qui sotto per renderlo persistente.
        </p>

        <div className="grid-3" style={{ marginBottom: 12 }}>
          <div className="field">
            <label htmlFor="input-gd-client-id">Google OAuth2 Client ID</label>
            <input
              id="input-gd-client-id"
              value={gdClientId}
              onChange={(e) => setGdClientId(e.target.value)}
              placeholder="xxxx.apps.googleusercontent.com"
            />
          </div>
          <div className="field">
            <label htmlFor="input-gd-client-secret">
              Google OAuth2 Client Secret {gdStatus?.has_client_secret ? '(già salvato)' : ''}
            </label>
            <input
              id="input-gd-client-secret"
              type="password"
              value={gdClientSecret}
              onChange={(e) => setGdClientSecret(e.target.value)}
              placeholder={gdStatus?.has_client_secret ? '•••••••• (lascia vuoto per mantenere)' : 'GOCSPX-...'}
            />
          </div>
          <div className="field">
            <label htmlFor="input-gd-redirect-uri">OAuth2 Redirect URI</label>
            <input
              id="input-gd-redirect-uri"
              value={gdRedirectUri}
              onChange={(e) => setGdRedirectUri(e.target.value)}
            />
          </div>
        </div>

        <div className="grid-3">
          <div className="field">
            <label htmlFor="input-gd-folder">Cartella Root Backup su Google Drive</label>
            <input
              id="input-gd-folder"
              value={gdFolder}
              onChange={(e) => setGdFolder(e.target.value)}
              placeholder="GOLDSMP-Server"
            />
          </div>
          <div className="field">
            <label htmlFor="input-gd-max-backups">Max Backup Mantenuti per Istanza</label>
            <input
              id="input-gd-max-backups"
              type="number"
              min={1}
              max={100}
              value={gdMaxBackups}
              onChange={(e) => setGdMaxBackups(Number(e.target.value))}
            />
          </div>
          <div className="field" style={{ justifyContent: 'center' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 18 }}>
              <input
                id="chk-gd-enabled"
                type="checkbox"
                checked={gdEnabled}
                onChange={(e) => setGdEnabled(e.target.checked)}
              />
              Sincronizza automaticamente ogni backup su Google Drive
            </label>
          </div>
        </div>
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
                id="btn-updater-check"
                type="button"
                className="btn btn-secondary btn-sm"
                disabled={checking}
                onClick={() => checkUpdates(false)}
              >
                <RefreshCw size={13} /> {checking ? 'Controllo...' : 'Controlla Ora'}
              </button>
              <button
                id="btn-updater-apply"
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
                  id="btn-updater-refresh"
                  type="button"
                  className="btn btn-success btn-sm"
                  onClick={() => window.location.reload()}
                >
                  <RefreshCw size={13} /> Aggiorna Pagina (F5)
                </button>
              )}
              {updStatus.binary_staged && (
                <button
                  id="btn-updater-restart"
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
              <label htmlFor="input-updater-repo">Repository GitHub (owner/repo)</label>
              <input
                id="input-updater-repo"
                value={updCfg.repo}
                onChange={(e) => setUpdCfg({ ...updCfg, repo: e.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="input-updater-interval">Intervallo Controllo (minuti)</label>
              <input
                id="input-updater-interval"
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
                  id="chk-updater-autocheck"
                  type="checkbox"
                  checked={updCfg.auto_check}
                  onChange={(e) => setUpdCfg({ ...updCfg, auto_check: e.target.checked })}
                />
                Controllo automatico in background
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                <input
                  id="chk-updater-autodownload"
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
              <label htmlFor="input-panel-name">Nome Pannello</label>
              <input
                id="input-panel-name"
                value={form.panel_name}
                onChange={(e) => setForm({ ...form, panel_name: e.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="input-accent-color">Colore Accento Principale</label>
              <div style={{ display: 'flex', gap: 8 }}>
                <input
                  id="input-accent-color-picker"
                  type="color"
                  style={{ width: 52, padding: 2 }}
                  value={form.accent_color}
                  onChange={(e) => setForm({ ...form, accent_color: e.target.value })}
                />
                <input
                  id="input-accent-color"
                  value={form.accent_color}
                  onChange={(e) => setForm({ ...form, accent_color: e.target.value })}
                />
              </div>
            </div>
            <div className="field">
              <label htmlFor="input-metrics-retention">Retention Metriche SQLite (Giorni)</label>
              <input
                id="input-metrics-retention"
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
            <span className="card-title">OIDC Single Sign-On (Authentik / Keycloak / Google OIDC)</span>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
              <input
                id="chk-oidc-enabled"
                type="checkbox"
                checked={form.oidc_enabled}
                onChange={(e) => setForm({ ...form, oidc_enabled: e.target.checked })}
              />
              Abilita Login OIDC SSO
            </label>
            <div className="field">
              <label htmlFor="input-oidc-issuer">Issuer URL</label>
              <input
                id="input-oidc-issuer"
                placeholder="https://accounts.google.com oppure https://auth.example.com/..."
                value={form.oidc_issuer_url}
                onChange={(e) => setForm({ ...form, oidc_issuer_url: e.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="input-oidc-client-id">Client ID</label>
              <input
                id="input-oidc-client-id"
                value={form.oidc_client_id}
                onChange={(e) => setForm({ ...form, oidc_client_id: e.target.value })}
              />
            </div>
            <div className="field">
              <label htmlFor="input-oidc-client-secret">Client Secret</label>
              <input
                id="input-oidc-client-secret"
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

