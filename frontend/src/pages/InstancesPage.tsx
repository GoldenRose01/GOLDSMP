import React, { useState } from 'react';
import { api, formatDuration } from '../api/client';
import { HostSystemInfo, Instance, SentinelStatus } from '../types';
import { getGameArt } from '../utils/gameArtwork';
import { formatRamPair, formatRamValue, useRamUnit } from '../utils/ramUnit';
import {
  Boxes,
  Cpu,
  Gamepad2,
  Globe,
  Play,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
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
  sentinel?: SentinelStatus | null;
  onRefresh: () => void;
  onSelectInstance: (id: string, tab?: InstanceTabId) => void;
  onOpenCreate: () => void;
  onOpenSentinel?: () => void;
  onNotify: (msg: string) => void;
}

export const InstancesPage: React.FC<InstancesPageProps> = ({
  instances,
  sysInfo,
  sentinel,
  onRefresh,
  onSelectInstance,
  onOpenCreate,
  onOpenSentinel,
  onNotify,
}) => {
  const [search, setSearch] = useState('');
  const [catFilter, setCatFilter] = useState('all');
  const [busyId, setBusyId] = useState<string | null>(null);
  const [quickRepoUrl, setQuickRepoUrl] = useState('');
  const [quickBranch, setQuickBranch] = useState('main');
  const [deployingRepo, setDeployingRepo] = useState(false);
  const [ramUnit, , toggleRamUnit] = useRamUnit();

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
          <h1>Istanze, Server & Coolify Sentinel</h1>
          <p>
            Gestione nativa in Rust • Watchdog Coolify Sentinel attivo • Horizon MC, Minecraft, SteamCMD & GitHub Web
          </p>
        </div>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          {onOpenSentinel && (
            <button
              id="btn-instances-open-sentinel"
              type="button"
              className="btn btn-secondary"
              onClick={onOpenSentinel}
            >
              <ShieldCheck size={16} color="var(--success)" />
              Sentinel ({sentinel?.agent_state || 'active'})
            </button>
          )}
          <button
            id="btn-open-create-instance"
            className="btn btn-primary"
            onClick={onOpenCreate}
            style={{
              padding: '10px 20px',
              fontSize: 13.5,
            }}
          >
            <Sparkles size={16} />
            + Crea Nuova Istanza
          </button>
        </div>
      </div>

      {/* 1-Click GitHub WebService Auto-Deploy Bar */}
      <form
        id="form-quick-github-deploy"
        onSubmit={handleQuickGithubDeploy}
        className="card"
        style={{
          marginBottom: 20,
          padding: '16px 18px',
          background:
            'linear-gradient(135deg, rgba(56, 189, 248, 0.06) 0%, rgba(245, 185, 50, 0.05) 55%, hsl(240, 12%, 10.5%) 100%)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
            <Globe size={19} color="var(--arc-cyan)" style={{ flexShrink: 0 }} />
            <div style={{ minWidth: 0 }}>
              <div style={{ fontFamily: 'var(--font-display)', fontWeight: 700, fontSize: 15, letterSpacing: '0.04em', textTransform: 'uppercase', color: '#fff6dc' }}>
                Auto-Deploy Web Service da Repository GitHub
              </div>
              <div style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                Incolla il link GitHub: GOLDSMP clona la repo, rileva lo stack (Node/Next/Vite, Python, Rust, Go, Static), installa le dipendenze e avvia il web server.
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', gap: 8, flex: '1 1 320px', justifyContent: 'flex-end', flexWrap: 'wrap', minWidth: 0 }}>
            <input
              id="input-github-repo-url"
              style={{ flex: '1 1 180px', minWidth: 0 }}
              placeholder="https://github.com/owner/repo..."
              value={quickRepoUrl}
              onChange={(e) => setQuickRepoUrl(e.target.value)}
            />
            <input
              id="input-github-branch"
              style={{ width: 88 }}
              placeholder="Branch"
              value={quickBranch}
              onChange={(e) => setQuickBranch(e.target.value)}
              title="Branch Git (default: main)"
            />
            <button
              id="btn-submit-github-deploy"
              type="submit"
              className="btn btn-primary"
              disabled={deployingRepo || !quickRepoUrl.trim()}
            >
              <Globe size={15} />
              {deployingRepo ? 'Configurazione...' : 'Crea & Avvia'}
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
        <div
          id="card-toggle-ram-unit"
          className="card"
          onClick={toggleRamUnit}
          style={{ cursor: 'pointer' }}
          title="Clicca per cambiare unità tra MB e GB"
        >
          <div className="kpi-value">
            {sysInfo
              ? formatRamPair(sysInfo.used_memory_mb, sysInfo.total_memory_mb, ramUnit)
              : '--'}
          </div>
          <div className="kpi-label">
            RAM Host (GOLDSMP:{' '}
            {sysInfo
              ? formatRamValue(sysInfo.panel_memory_mb, ramUnit, ramUnit === 'GB' ? 3 : 1)
              : '--'}
            ) • [{ramUnit}]
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
            id="input-filter-instances"
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
              id={`btn-filter-cat-${c.id}`}
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
          const sentHealth = sentinel?.instances?.find((x) => x.instance_id === inst.id);

          return (
            <div
              id={`instance-card-${inst.id}`}
              key={inst.id}
              className="instance-card"
              onClick={() => onSelectInstance(inst.id)}
              style={{
                cursor: 'pointer',
                overflow: 'hidden',
                position: 'relative',
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
                  backgroundSize: '230px auto',
                  opacity: isRunning ? 0.72 : 0.44,
                  pointerEvents: 'none',
                  transition: 'opacity 0.25s ease',
                }}
              />

              <div className="instance-card-top" style={{ position: 'relative', zIndex: 1 }}>
                <div style={{ display: 'flex', gap: 11, minWidth: 0 }}>
                  <div
                    className="instance-icon-box"
                    style={{
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
                        fontSize: 15,
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        color: '#fff6dc',
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
                        flexWrap: 'wrap',
                      }}
                    >
                      <span style={{ color: art.accent, fontWeight: 600 }}>{art.title}</span>
                      <span>• :{inst.port}</span>
                      {sentHealth && isRunning && (
                        <span
                          style={{
                            color: sentHealth.port_reachable ? 'var(--success)' : 'var(--warning)',
                            fontSize: 10.5,
                          }}
                          title="Probe TCP Coolify Sentinel"
                        >
                          • {sentHealth.port_reachable ? `${sentHealth.latency_ms ?? 1}ms` : 'TCP wait'}
                        </span>
                      )}
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
                    <span>RAM ({ramUnit})</span>
                    <span>
                      {formatRamPair(inst.memory_mb, inst.max_memory_mb, ramUnit)}
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
                  flexWrap: 'wrap',
                }}
              >
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 10,
                    fontSize: 12,
                    color: 'var(--text-secondary)',
                  }}
                >
                  <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                    <Users size={13} /> {inst.active_players.length}/{inst.max_players}
                  </span>
                  {isRunning && <span>Up: {formatDuration(inst.uptime_secs)}</span>}
                </div>

                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                  {isRunning ? (
                    <button
                      id={`btn-stop-instance-${inst.id}`}
                      className="btn btn-danger btn-sm"
                      disabled={busyId === inst.id}
                      onClick={(e) => handleAction(e, inst.id, 'stop')}
                    >
                      <Square size={12} /> Stop
                    </button>
                  ) : (
                    <button
                      id={`btn-start-instance-${inst.id}`}
                      className="btn btn-success btn-sm"
                      disabled={busyId === inst.id}
                      onClick={(e) => handleAction(e, inst.id, 'start')}
                    >
                      <Play size={12} /> Avvia
                    </button>
                  )}
                  <button
                    id={`btn-console-instance-${inst.id}`}
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
                    id={`btn-manage-instance-${inst.id}`}
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

        {/* Always-Visible "Crea Nuova Istanza" Hero Card */}
        <button
          id="card-cta-create-instance"
          type="button"
          onClick={onOpenCreate}
          className="instance-card"
          style={{
            border: '1px dashed var(--border-strong)',
            background:
              'radial-gradient(circle at 50% 35%, rgba(245, 185, 50, 0.08), hsl(240, 13%, 9.5%))',
            alignItems: 'center',
            justifyContent: 'center',
            textAlign: 'center',
            minHeight: 195,
            gap: 10,
          }}
        >
          <div
            style={{
              width: 46,
              height: 46,
              borderRadius: 11,
              background: 'linear-gradient(145deg, #ffe082, #f5b932)',
              color: '#140e02',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 6px 18px rgba(0, 0, 0, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.5)',
            }}
          >
            <Plus size={22} strokeWidth={2.5} />
          </div>
          <div
            style={{
              fontFamily: 'var(--font-display)',
              fontSize: 17.5,
              fontWeight: 700,
              letterSpacing: '0.04em',
              textTransform: 'uppercase',
              color: '#fff6dc',
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
