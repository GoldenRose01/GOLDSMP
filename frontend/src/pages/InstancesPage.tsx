import React, { useState } from 'react';
import { api, formatDuration } from '../api/client';
import { HostSystemInfo, Instance } from '../types';
import {
  Boxes,
  Cpu,
  Gamepad2,
  Play,
  Plus,
  Search,
  Settings2,
  Square,
  Users,
} from 'lucide-react';

interface InstancesPageProps {
  instances: Instance[];
  sysInfo: HostSystemInfo | null;
  onRefresh: () => void;
  onSelectInstance: (id: string) => void;
  onOpenCreate: () => void;
  onNotify: (msg: string) => void;
}

export const InstancesPage: React.FC<InstancesPageProps> = ({
  instances,
  sysInfo,
  onRefresh,
  onSelectInstance,
  onOpenCreate,
  onNotify,
}) => {
  const [search, setSearch] = useState('');
  const [catFilter, setCatFilter] = useState('all');
  const [busyId, setBusyId] = useState<string | null>(null);

  const runningCount = instances.filter((i) => i.status === 'running').length;
  const totalPlayers = instances.reduce((acc, i) => acc + i.active_players.length, 0);

  const filtered = instances.filter((i) => {
    const matchCat = catFilter === 'all' || i.category === catFilter;
    const matchSearch =
      i.name.toLowerCase().includes(search.toLowerCase()) ||
      i.template_id.toLowerCase().includes(search.toLowerCase());
    return matchCat && matchSearch;
  });

  const handleAction = async (e: React.MouseEvent, id: string, action: 'start' | 'stop') => {
    e.stopPropagation();
    setBusyId(id);
    try {
      await api(`/api/instances/${id}/${action}`, { method: 'POST' });
      onNotify(action === 'start' ? 'Istanza avviata' : 'Arresto inviato');
      onRefresh();
    } catch (err: any) {
      onNotify(`Errore: ${err.message}`);
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Istanze & Server</h1>
          <p>Gestione nativa ad alte prestazioni • Nessun limite di licenza sulle istanze</p>
        </div>
        <button className="btn btn-primary" onClick={onOpenCreate}>
          <Plus size={16} />
          Crea Istanza
        </button>
      </div>

      {/* Host Summary KPIs */}
      <div className="grid-4" style={{ marginBottom: 22 }}>
        <div className="card">
          <div className="kpi-value">
            {runningCount} / {instances.length}
          </div>
          <div className="kpi-label">Istanze Attive (Illimitate)</div>
        </div>
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--accent)' }}>
            {sysInfo ? `${sysInfo.cpu_usage_percent.toFixed(1)}%` : '--'}
          </div>
          <div className="kpi-label">
            CPU Host ({sysInfo?.cpu_cores || 0} core)
          </div>
        </div>
        <div className="card">
          <div className="kpi-value">
            {sysInfo
              ? `${(sysInfo.used_memory_mb / 1024).toFixed(1)} / ${(sysInfo.total_memory_mb / 1024).toFixed(1)} GB`
              : '--'}
          </div>
          <div className="kpi-label">
            RAM Host (GOLDSMP: {sysInfo ? `${sysInfo.panel_memory_mb.toFixed(1)} MB` : '--'})
          </div>
        </div>
        <div className="card">
          <div className="kpi-value" style={{ color: 'var(--success)' }}>
            {totalPlayers}
          </div>
          <div className="kpi-label">Giocatori Online Totali</div>
        </div>
      </div>

      {/* Filter Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 12,
          marginBottom: 18,
          flexWrap: 'wrap',
        }}
      >
        <div style={{ position: 'relative', minWidth: 260, flex: '0 1 340px' }}>
          <Search
            size={15}
            style={{
              position: 'absolute',
              left: 11,
              top: '50%',
              transform: 'translateY(-50%)',
              color: 'var(--text-muted)',
            }}
          />
          <input
            style={{ width: '100%', paddingLeft: 34 }}
            placeholder="Cerca istanza o template..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>

        <div className="pill-group">
          {[
            { id: 'all', label: 'Tutte' },
            { id: 'minecraft', label: 'Minecraft' },
            { id: 'steamcmd', label: 'SteamCMD' },
            { id: 'generic', label: 'Generiche' },
          ].map((c) => (
            <button
              key={c.id}
              className={`pill-btn ${catFilter === c.id ? 'active' : ''}`}
              onClick={() => setCatFilter(c.id)}
            >
              {c.label}
            </button>
          ))}
        </div>
      </div>

      {/* Instances Grid */}
      {filtered.length === 0 ? (
        <div className="card" style={{ textAlign: 'center', padding: 48 }}>
          <p style={{ color: 'var(--text-secondary)', marginBottom: 14 }}>
            Nessuna istanza trovata. Crea la tua prima istanza Minecraft, SteamCMD o Node/Python!
          </p>
          <button className="btn btn-primary" onClick={onOpenCreate}>
            <Plus size={16} /> Crea Prima Istanza
          </button>
        </div>
      ) : (
        <div className="grid-3">
          {filtered.map((inst) => {
            const memPct = inst.max_memory_mb
              ? Math.min(100, (inst.memory_mb / inst.max_memory_mb) * 100)
              : 0;
            const isRunning = inst.status === 'running' || inst.status === 'starting';

            return (
              <div
                key={inst.id}
                className="instance-card"
                onClick={() => onSelectInstance(inst.id)}
                style={{ cursor: 'pointer' }}
              >
                <div className="instance-card-top">
                  <div style={{ display: 'flex', gap: 12, minWidth: 0 }}>
                    <div className="instance-icon-box">
                      {inst.category === 'minecraft' && <Boxes size={20} />}
                      {inst.category === 'steamcmd' && <Gamepad2 size={20} />}
                      {inst.category === 'generic' && <Cpu size={20} />}
                    </div>
                    <div style={{ minWidth: 0 }}>
                      <div
                        style={{
                          fontWeight: 600,
                          fontSize: 15,
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                        }}
                      >
                        {inst.name}
                      </div>
                      <div style={{ fontSize: 12, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                        {inst.template_id} • :{inst.port}
                      </div>
                    </div>
                  </div>

                  <span className={`status-badge status-${inst.status}`}>
                    <span className="status-dot" />
                    {inst.status}
                  </span>
                </div>

                {/* Live Resource Bars */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: 9 }}>
                  <div className="progress-row">
                    <div className="progress-meta">
                      <span>CPU</span>
                      <span>{inst.cpu_percent.toFixed(1)}%</span>
                    </div>
                    <div className="progress-track">
                      <div
                        className="progress-fill"
                        style={{ width: `${Math.min(100, inst.cpu_percent)}%` }}
                      />
                    </div>
                  </div>
                  <div className="progress-row">
                    <div className="progress-meta">
                      <span>RAM</span>
                      <span>
                        {inst.memory_mb.toFixed(0)} / {inst.max_memory_mb} MB
                      </span>
                    </div>
                    <div className="progress-track">
                      <div
                        className="progress-fill"
                        style={{
                          width: `${memPct}%`,
                          background: memPct > 85 ? 'var(--danger)' : 'var(--success)',
                        }}
                      />
                    </div>
                  </div>
                </div>

                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    paddingTop: 10,
                    borderTop: '1px solid var(--border-subtle)',
                    gap: 8,
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: 12, fontSize: 12, color: 'var(--text-secondary)' }}>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                      <Users size={13} /> {inst.active_players.length}/{inst.max_players}
                    </span>
                    {isRunning && <span>Up: {formatDuration(inst.uptime_secs)}</span>}
                  </div>

                  <div style={{ display: 'flex', gap: 6 }}>
                    {isRunning ? (
                      <button
                        className="btn btn-danger btn-sm"
                        disabled={busyId === inst.id}
                        onClick={(e) => handleAction(e, inst.id, 'stop')}
                      >
                        <Square size={12} /> Stop
                      </button>
                    ) : (
                      <button
                        className="btn btn-success btn-sm"
                        disabled={busyId === inst.id}
                        onClick={(e) => handleAction(e, inst.id, 'start')}
                      >
                        <Play size={12} /> Avvia
                      </button>
                    )}
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelectInstance(inst.id);
                      }}
                    >
                      <Settings2 size={13} /> Gestisci
                    </button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
