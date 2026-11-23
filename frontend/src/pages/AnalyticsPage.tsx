import React, { useEffect, useState } from 'react';
import { api, formatDuration, getToken } from '../api/client';
import { AnalyticsOverview, GeoFilterRule, Instance, MetricPoint } from '../types';
import { SparkChart } from '../components/SparkChart';
import { useRamUnit } from '../utils/ramUnit';
import { Download, Globe, ShieldAlert, Trash2, Users } from 'lucide-react';

interface AnalyticsPageProps {
  instances: Instance[];
  onNotify: (msg: string) => void;
}

export const AnalyticsPage: React.FC<AnalyticsPageProps> = ({ instances, onNotify }) => {
  const [target, setTarget] = useState('host');
  const [range, setRange] = useState('1h');
  const [metrics, setMetrics] = useState<MetricPoint[]>([]);
  const [overview, setOverview] = useState<AnalyticsOverview | null>(null);
  const [geoRules, setGeoRules] = useState<GeoFilterRule[]>([]);
  const [ramUnit] = useRamUnit();

  const [ruleCountryCode, setRuleCountryCode] = useState('RU');
  const [ruleCountryName, setRuleCountryName] = useState('Russia');
  const [ruleAction, setRuleAction] = useState<'block' | 'allow'>('block');
  const [ruleInstance, setRuleInstance] = useState('*');

  const loadAll = async () => {
    try {
      const [m, o, g] = await Promise.all([
        api<MetricPoint[]>(
          `/api/analytics/metrics?instance_id=${encodeURIComponent(target)}&range=${range}`
        ),
        api<AnalyticsOverview>('/api/analytics/overview'),
        api<GeoFilterRule[]>('/api/analytics/geo-rules'),
      ]);
      setMetrics(m);
      setOverview(o);
      setGeoRules(g);
    } catch {}
  };

  useEffect(() => {
    loadAll();
    const timer = setInterval(loadAll, 5000);
    return () => clearInterval(timer);
  }, [target, range]);

  const exportCsv = () => {
    const token = getToken() || '';
    window.open(`/api/analytics/export.csv?token=${encodeURIComponent(token)}`, '_blank');
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
      <div className="page-header" style={{ marginBottom: 4 }}>
        <div>
          <h1>Analytics, Telemetria & Geo-Filtering</h1>
          <p>
            Tracciamento completo di risorse, sessioni giocatori, playtime, IP e geolocalizzazione — 100% incluso senza licenze Premium.
          </p>
        </div>
        <button className="btn btn-secondary" onClick={exportCsv}>
          <Download size={15} /> Esporta CSV Sessioni
        </button>
      </div>

      {/* Player Analytics KPIs */}
      <div className="grid-4">
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--accent)' }}>
            {overview?.unique_players ?? 0}
          </div>
          <div className="kpi-label">Giocatori Unici Tracciati</div>
        </div>
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--success)' }}>
            {overview?.active_sessions ?? 0}
          </div>
          <div className="kpi-label">Sessioni Attive Ora</div>
        </div>
        <div className="card">
          <div className="kpi-value">
            {formatDuration(overview?.total_playtime_secs ?? 0)}
          </div>
          <div className="kpi-label">
            Playtime Totale (Media: {formatDuration(overview?.avg_session_secs ?? 0)})
          </div>
        </div>
        <div className="card">
          <div className="kpi-value">{overview?.countries_count ?? 0}</div>
          <div className="kpi-label">Paesi Rilevati</div>
        </div>
      </div>

      {/* Resource Time-Series Controls */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ fontSize: 13, color: 'var(--text-secondary)', fontWeight: 500 }}>Target Telemetria:</span>
          <select value={target} onChange={(e) => setTarget(e.target.value)}>
            <option value="host">Sistema Host Completo</option>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                Istanza: {i.name}
              </option>
            ))}
          </select>
        </div>

        <div className="pill-group">
          {['15m', '1h', '6h', '24h', '7d'].map((r) => (
            <button
              key={r}
              className={`pill-btn ${range === r ? 'active' : ''}`}
              onClick={() => setRange(r)}
            >
              {r}
            </button>
          ))}
        </div>
      </div>

      <div className="grid-2">
        <SparkChart
          title="Utilizzo CPU nel Tempo"
          unit="%"
          data={metrics.map((m) => ({ timestamp: m.timestamp, value: m.cpu_percent }))}
          maxOverride={100}
        />
        <SparkChart
          title={`Memoria RAM nel Tempo (${ramUnit})`}
          unit={ramUnit}
          color="#10b981"
          data={metrics.map((m) => ({
            timestamp: m.timestamp,
            value: ramUnit === 'GB' ? Number((m.memory_mb / 1024).toFixed(2)) : m.memory_mb,
          }))}
        />
        <SparkChart
          title="Traffico di Rete (RX KB/s)"
          unit="KB"
          color="#38bdf8"
          data={metrics.map((m) => ({ timestamp: m.timestamp, value: m.net_rx_kb }))}
        />
        <SparkChart
          title="Giocatori Online nel Tempo"
          unit="pl"
          color="#f59e0b"
          data={metrics.map((m) => ({ timestamp: m.timestamp, value: m.players_online }))}
        />
      </div>

      {/* Leaderboard & Geo Breakdown */}
      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Users size={16} color="var(--accent)" /> Classifica Playtime Giocatori
            </span>
          </div>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Giocatore</th>
                  <th>Paese</th>
                  <th>Sessioni</th>
                  <th>Tempo Totale</th>
                </tr>
              </thead>
              <tbody>
                {(overview?.leaderboard || []).map((p) => (
                  <tr key={p.player_name}>
                    <td style={{ fontWeight: 600 }}>{p.player_name}</td>
                    <td>{p.country_code} ({p.country_name})</td>
                    <td>{p.total_sessions}</td>
                    <td style={{ fontFamily: 'var(--font-mono)' }}>{formatDuration(p.total_playtime_secs)}</td>
                  </tr>
                ))}
                {(overview?.leaderboard || []).length === 0 && (
                  <tr>
                    <td colSpan={4} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                      Nessun giocatore registrato. Avvia la Demo Node.js e digita "simjoin Alex 79.12.45.8" in console!
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <span className="card-title">
              <Globe size={16} color="var(--accent)" /> Distribuzione Geografica
            </span>
          </div>
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Paese</th>
                  <th>Giocatori Unici</th>
                  <th>Sessioni</th>
                  <th>Playtime</th>
                </tr>
              </thead>
              <tbody>
                {(overview?.geo_stats || []).map((g) => (
                  <tr key={g.country_code}>
                    <td style={{ fontWeight: 600 }}>
                      {g.country_code} — {g.country_name}
                    </td>
                    <td>{g.unique_players}</td>
                    <td>{g.total_sessions}</td>
                    <td style={{ fontFamily: 'var(--font-mono)' }}>{formatDuration(g.total_playtime_secs)}</td>
                  </tr>
                ))}
                {(overview?.geo_stats || []).length === 0 && (
                  <tr>
                    <td colSpan={4} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                      Nessun dato geografico ancora raccolto.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      {/* Geo-Filtering Rules (AMP Advanced/Enterprise feature unlocked!) */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">
            <ShieldAlert size={16} color="var(--warning)" /> Regole Geo-Filtering (Auto-Kick per Paese)
          </span>
        </div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 14, flexWrap: 'wrap' }}>
          <select value={ruleInstance} onChange={(e) => setRuleInstance(e.target.value)}>
            <option value="*">Tutte le istanze (*)</option>
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name}
              </option>
            ))}
          </select>
          <input
            style={{ width: 90 }}
            placeholder="ISO (IT)"
            value={ruleCountryCode}
            onChange={(e) => setRuleCountryCode(e.target.value)}
          />
          <input
            placeholder="Nome Paese"
            value={ruleCountryName}
            onChange={(e) => setRuleCountryName(e.target.value)}
          />
          <select value={ruleAction} onChange={(e) => setRuleAction(e.target.value as any)}>
            <option value="block">Blocca (Kick automatico)</option>
            <option value="allow">Consenti</option>
          </select>
          <button
            className="btn btn-primary btn-sm"
            onClick={async () => {
              if (!ruleCountryCode.trim()) return;
              await api('/api/analytics/geo-rules', {
                method: 'POST',
                body: JSON.stringify({
                  instance_id: ruleInstance,
                  country_code: ruleCountryCode.trim().toUpperCase(),
                  country_name: ruleCountryName.trim() || ruleCountryCode.trim().toUpperCase(),
                  action: ruleAction,
                }),
              });
              onNotify('Regola Geo-Filter aggiunta');
              loadAll();
            }}
          >
            Aggiungi Regola
          </button>
        </div>

        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Scope</th>
                <th>Paese</th>
                <th>Azione</th>
                <th style={{ textAlign: 'right' }}>Rimuovi</th>
              </tr>
            </thead>
            <tbody>
              {geoRules.map((r) => (
                <tr key={r.id}>
                  <td>{r.instance_id === '*' ? 'Globale' : r.instance_id}</td>
                  <td style={{ fontWeight: 600 }}>
                    {r.country_code} — {r.country_name}
                  </td>
                  <td>
                    <span
                      className={`status-badge ${
                        r.action === 'block' ? 'status-crashed' : 'status-running'
                      }`}
                    >
                      {r.action}
                    </span>
                  </td>
                  <td style={{ textAlign: 'right' }}>
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={async () => {
                        await api(`/api/analytics/geo-rules/${r.id}`, { method: 'DELETE' });
                        loadAll();
                      }}
                    >
                      <Trash2 size={13} />
                    </button>
                  </td>
                </tr>
              ))}
              {geoRules.length === 0 && (
                <tr>
                  <td colSpan={4} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                    Nessuna regola di geo-filtering configurata.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Recent Player Sessions */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">Cronologia Sessioni Recenti</span>
        </div>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Giocatore</th>
                <th>Istanza</th>
                <th>IP</th>
                <th>Paese</th>
                <th>Ingresso</th>
                <th>Durata / Stato</th>
              </tr>
            </thead>
            <tbody>
              {(overview?.recent_sessions || []).map((s) => (
                <tr key={s.id}>
                  <td style={{ fontWeight: 600 }}>{s.player_name}</td>
                  <td>{s.instance_name}</td>
                  <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{s.ip_address}</td>
                  <td>{s.country_code} ({s.country_name})</td>
                  <td style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                    {new Date(s.joined_at).toLocaleString()}
                  </td>
                  <td>
                    {s.left_at ? (
                      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                        {formatDuration(s.duration_secs)}
                      </span>
                    ) : (
                      <span className="status-badge status-running">
                        <span className="status-dot" /> Online
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
