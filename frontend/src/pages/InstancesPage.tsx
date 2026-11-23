import React, { useState } from 'react';
import { api, formatDuration } from '../api/client';
import { HostSystemInfo, Instance } from '../types';
import { getGameArt } from '../utils/gameArtwork';
import {
  Boxes,
  Cpu,
  Gamepad2,
  Globe,
  Play,
  Plus,
  Search,
  Settings2,
  Sparkles,
  Square,
  Terminal,
  Users,
  Zap,
} from 'lucide-react';
import { InstanceTabId } from './InstanceDetailPage';

interface InstancesPageProps {
  instances: Instance[];
  sysInfo: HostSystemInfo | null;
  onRefresh: () => void;
  onSelectInstance: (id: string, tab?: InstanceTabId) => void;
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
  const [quickRepoUrl, setQuickRepoUrl] = useState('');
  const [quickBranch, setQuickBranch] = useState('main');
  const [deployingRepo, setDeployingRepo] = useState(false);

  const runningCount = instances.filter((i) => i.status === 'running').length;
  const totalPlayers = instances.reduce((acc, i) => acc + i.active_players.length, 0);

  const filtered = instances.filter((i) => {
    const matchCat = catFilter === 'all' || i.category === catFilter;
    const matchSearch =
      i.name.toLowerCase().includes(search.toLowerCase()) ||
      i.template_id.toLowerCase().includes(search.toLowerCase());
    return matchCat && matchSearch;
  });

  const handleQuickGithubDeploy = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!quickRepoUrl.trim()) return;
    setDeployingRepo(true);
    try {
      const created = await api<Instance>('/api/instances/github-deploy', {
        method: 'POST',
        body: JSON.stringify({
          repo_url: quickRepoUrl.trim(),
          branch: quickBranch.trim() || 'main',
          auto_start: true,
        }),
      });
      setQuickRepoUrl('');
      onNotify(`Web Service "${created.name}" creato e avviato automaticamente sulla porta :${created.port}!`);
      onRefresh();
      onSelectInstance(created.id);
    } catch (err: any) {
      onNotify(`Errore deploy GitHub: ${err.message}`);
    } finally {
      setDeployingRepo(false);
    }
  };

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
      {/* High-Visibility Header with Prominent Create Button */}
      <div className="page-header">
        <div>
          <h1>Istanze, Server & GitHub Web Services</h1>
          <p>
            Gestione nativa in Rust • Horizon MC, Minecraft, SteamCMD & Auto-Deploy da link GitHub
          </p>
        </div>
        <button
          className="btn btn-primary"
          onClick={onOpenCreate}
          style={{
            padding: '11px 22px',
            fontSize: 14,
            boxShadow: '0 0 24px rgba(245, 185, 50, 0.45)',
            border: '1px solid rgba(255, 240, 179, 0.65)',
          }}
        >
          <Sparkles size={16} />
          + Crea Nuova Istanza
        </button>
      </div>

      {/* 1-Click GitHub WebService Auto-Deploy Bar */}
      <form
        onSubmit={handleQuickGithubDeploy}
        className="card"
        style={{
          marginBottom: 20,
          padding: '14px 18px',
          border: '1px solid rgba(56, 189, 248, 0.35)',
          background:
            'linear-gradient(90deg, rgba(56, 189, 248, 0.1) 0%, rgba(245, 185, 50, 0.08) 60%, rgba(15, 15, 22, 0.95) 100%)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <Globe size={19} color="var(--arc-cyan)" />
            <div>
              <div style={{ fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: 15, letterSpacing: '0.04em', textTransform: 'uppercase', color: '#fff4d2' }}>
                Auto-Deploy Web Service da Repository GitHub
              </div>
              <div style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                Incolla il link GitHub: GOLDSMP clona la repo, rileva lo stack (Node/Next/Vite, Python, Rust, Go, Static), installa le dipendenze e avvia il web server.
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', gap: 8, flex: '1 1 420px', justifyContent: 'flex-end', flexWrap: 'wrap' }}>
            <input
              style={{ flex: 1, minWidth: 250 }}
              placeholder="https://github.com/owner/repo oppure owner/repo..."
              value={quickRepoUrl}
              onChange={(e) => setQuickRepoUrl(e.target.value)}
            />
            <input
              style={{ width: 95 }}
              placeholder="Branch"
              value={quickBranch}
              onChange={(e) => setQuickBranch(e.target.value)}
              title="Branch Git (default: main)"
            />
            <button
              type="submit"
              className="btn btn-primary"
              disabled={deployingRepo || !quickRepoUrl.trim()}
            >
              <Globe size={15} />
              {deployingRepo ? 'Configurazione...' : 'Crea & Avvia Web Service'}
            </button>
          </div>
        </div>
      </form>

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
          <div className="kpi-label">CPU Host ({sysInfo?.cpu_cores || 0} core)</div>
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
          <div className="kpi-value" style={{ color: 'var(--arc-cyan)' }}>
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
            { id: 'horizon', label: 'Horizon MC (Rust)' },
            { id: 'minecraft', label: 'Minecraft' },
            { id: 'steamcmd', label: 'SteamCMD' },
            { id: 'webservice', label: 'GitHub Web' },
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

      {/* Instances Grid + Dedicated Create New Instance CTA Card */}
      <div className="grid-3">
        {filtered.map((inst) => {
          const memPct = inst.max_memory_mb
            ? Math.min(100, (inst.memory_mb / inst.max_memory_mb) * 100)
            : 0;
          const isRunning = inst.status === 'running' || inst.status === 'starting';
          const art = getGameArt(inst.template_id, inst.category);

          return (
            <div
              key={inst.id}
              className="instance-card"
              onClick={() => onSelectInstance(inst.id)}
              style={{
                cursor: 'pointer',
                overflow: 'hidden',
                position: 'relative',
                borderLeft: `4px solid ${
                  inst.status === 'running'
                    ? 'var(--arc-cyan)'
                    : inst.status === 'starting' || inst.status === 'stopping'
                    ? 'var(--accent)'
                    : inst.status === 'crashed'
                    ? 'var(--danger)'
                    : 'rgba(148, 163, 184, 0.28)'
                }`,
              }}
            >
              {/* Transparent Game Artwork Watermark inside Instance Card */}
              <div
                style={{
                  position: 'absolute',
                  inset: 0,
                  backgroundImage: `url("${art.bgSvgDataUri}")`,
                  backgroundRepeat: 'no-repeat',
                  backgroundPosition: 'right -12px center',
                  backgroundSize: '250px auto',
                  opacity: isRunning ? 0.85 : 0.55,
                  pointerEvents: 'none',
                  transition: 'opacity 0.25s ease',
                }}
              />

              <div className="instance-card-top" style={{ position: 'relative', zIndex: 1 }}>
                <div style={{ display: 'flex', gap: 12, minWidth: 0 }}>
                  <div
                    className="instance-icon-box"
                    style={{
                      borderColor: `${art.accent}66`,
                      color: art.accent,
                    }}
                  >
                    {inst.category === 'horizon' && <Zap size={20} />}
                    {inst.category === 'minecraft' && <Boxes size={20} />}
                    {inst.category === 'steamcmd' && <Gamepad2 size={20} />}
                    {inst.category === 'webservice' && <Globe size={20} />}
                    {inst.category === 'generic' && <Cpu size={20} />}
                  </div>
                  <div style={{ minWidth: 0 }}>
                    <div
                      style={{
                        fontWeight: 700,
                        fontSize: 15.5,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        color: '#fff4d2',
                      }}
                    >
                      {inst.name}
                    </div>
                    <div
                      style={{
                        fontSize: 11.5,
                        color: 'var(--text-secondary)',
                        fontFamily: 'var(--font-mono)',
                        display: 'flex',
                        alignItems: 'center',
                        gap: 6,
                      }}
                    >
                      <span style={{ color: art.accent, fontWeight: 600 }}>{art.title}</span>
                      <span>• :{inst.port}</span>
                    </div>
                  </div>
                </div>

                <span className={`status-badge status-${inst.status}`}>
                  <span className="status-dot" />
                  {inst.status}
                </span>
              </div>

              {/* Live Resource Bars */}
              <div
                style={{
                  position: 'relative',
                  zIndex: 1,
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 9,
                }}
              >
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
                  position: 'relative',
                  zIndex: 1,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  paddingTop: 10,
                  borderTop: '1px solid var(--border-subtle)',
                  gap: 8,
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 12,
                    fontSize: 12,
                    color: 'var(--text-secondary)',
                  }}
                >
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
                    title="Apri Console Live"
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelectInstance(inst.id, 'console');
                    }}
                  >
                    <Terminal size={13} />
                  </button>
                  <button
                    className="btn btn-primary btn-sm"
                    onClick={(e) => {
                      e.stopPropagation();
                      onSelectInstance(inst.id, 'status');
                    }}
                  >
                    <Settings2 size={13} /> Gestisci
                  </button>
                </div>
              </div>
            </div>
          );
        })}

        {/* Always-Visible "Crea Nuova Istanza" Gold Hero Card */}
        <button
          type="button"
          onClick={onOpenCreate}
          className="instance-card"
          style={{
            border: '2px dashed rgba(245, 185, 50, 0.5)',
            background:
              'radial-gradient(circle at 50% 35%, rgba(245, 185, 50, 0.14), rgba(15, 15, 22, 0.9))',
            alignItems: 'center',
            justifyContent: 'center',
            textAlign: 'center',
            minHeight: 195,
            gap: 10,
          }}
        >
          <div
            style={{
              width: 48,
              height: 48,
              borderRadius: 12,
              background: 'linear-gradient(135deg, #ffd466, #f5b932)',
              color: '#160f02',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 0 22px rgba(245, 185, 50, 0.45)',
            }}
          >
            <Plus size={24} strokeWidth={2.5} />
          </div>
          <div
            style={{
              fontFamily: 'var(--font-display)',
              fontSize: 18,
              fontWeight: 700,
              letterSpacing: '0.05em',
              textTransform: 'uppercase',
              color: '#fff4d2',
            }}
          >
            + Crea Nuova Istanza
          </div>
          <div style={{ fontSize: 12.5, color: 'var(--text-secondary)', maxWidth: 250 }}>
            Dispiega in 1 click un server Horizon MC (Rust), Minecraft Java/Bedrock, SteamCMD o App Nativa
          </div>
        </button>
      </div>
    </div>
  );
};
