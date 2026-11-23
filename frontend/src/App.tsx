import React, { useEffect, useMemo, useState } from 'react';
import { api, getToken, setToken } from './api/client';
import { AppTemplate, HostSystemInfo, Instance, PanelSettings, UpdateStatus, User } from './types';
import { LoginView } from './components/LoginView';
import { CreateInstanceModal } from './components/CreateInstanceModal';
import { InstancesPage } from './pages/InstancesPage';
import { InstanceDetailPage, InstanceTabId } from './pages/InstanceDetailPage';
import { AnalyticsPage } from './pages/AnalyticsPage';
import { AuditLogPage, SettingsPage, UsersPage, WebhooksPage } from './pages/OpsPages';
import { formatRamValue, useRamUnit } from './utils/ramUnit';
import {
  Activity,
  Archive,
  ArrowLeft,
  BarChart3,
  Bell,
  CalendarClock,
  FileClock,
  Folder,
  Globe,
  HardDrive,
  LogOut,
  Package,
  Plus,
  RefreshCw,
  Search,
  Server,
  Settings,
  Terminal,
  Users,
  Webhook,
  Zap,
} from 'lucide-react';

type NavPage = 'instances' | 'analytics' | 'webhooks' | 'users' | 'audit' | 'settings';

export function App() {
  const [user, setUser] = useState<User | null>(null);
  const [authChecked, setAuthChecked] = useState(false);
  const [settings, setSettings] = useState<PanelSettings | null>(null);
  const [sysInfo, setSysInfo] = useState<HostSystemInfo | null>(null);
  const [instances, setInstances] = useState<Instance[]>([]);
  const [templates, setTemplates] = useState<AppTemplate[]>([]);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus | null>(null);

  const [page, setPage] = useState<NavPage>('instances');
  const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);
  const [instanceTab, setInstanceTab] = useState<InstanceTabId>('status');
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  // AMP-style Global Quick Search & RAM Unit (MB <-> GB)
  const [quickQuery, setQuickQuery] = useState('');
  const [quickOpen, setQuickOpen] = useState(false);
  const [ramUnit, setRamUnit, toggleRamUnit] = useRamUnit();

  const notify = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3200);
  };

  useEffect(() => {
    api<PanelSettings>('/api/settings')
      .then((s) => {
        setSettings(s);
        if (s.accent_color) {
          document.documentElement.style.setProperty('--accent', s.accent_color);
        }
      })
      .catch(() => {});

    if (getToken()) {
      api<User>('/api/auth/me')
        .then((u) => setUser(u))
        .catch(() => setToken(null))
        .finally(() => setAuthChecked(true));
    } else {
      setAuthChecked(true);
    }
  }, []);

  // Keyboard shortcut Ctrl+K / Cmd+K for AMP-style quick jump
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setQuickOpen((v) => !v);
      } else if (e.key === 'Escape') {
        setQuickOpen(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const refreshCore = async () => {
    if (!user) return;
    try {
      const [insts, info, upd] = await Promise.all([
        api<Instance[]>('/api/instances'),
        api<HostSystemInfo>('/api/system/info'),
        api<UpdateStatus>('/api/updater/status'),
      ]);
      setInstances(insts);
      setSysInfo(info);
      setUpdateStatus(upd);
    } catch {}
  };

  useEffect(() => {
    if (!user) return;
    api<AppTemplate[]>('/api/templates').then(setTemplates).catch(() => {});
    refreshCore();
    const timer = setInterval(refreshCore, 4000);
    return () => clearInterval(timer);
  }, [user]);

  const selectedInstance = useMemo(
    () => instances.find((i) => i.id === selectedInstanceId) || null,
    [instances, selectedInstanceId]
  );

  const openInstance = (id: string, tab: InstanceTabId = 'status') => {
    setSelectedInstanceId(id);
    setInstanceTab(tab);
    setQuickOpen(false);
    setQuickQuery('');
  };

  if (!authChecked) {
    return null;
  }

  if (!user) {
    return <LoginView settings={settings} onLogin={(u) => setUser(u)} />;
  }

  const navItems: { id: NavPage; label: string; icon: React.ElementType }[] = [
    { id: 'instances', label: 'Istanze', icon: Server },
    { id: 'analytics', label: 'Analytics & Geo', icon: BarChart3 },
    { id: 'webhooks', label: 'Webhook', icon: Webhook },
    { id: 'users', label: 'Utenti & Ruoli', icon: Users },
    { id: 'audit', label: 'Audit Log', icon: FileClock },
    { id: 'settings', label: 'Impostazioni & Update', icon: Settings },
  ];

  // AMP-style Instance Sidebar Menu items
  const instanceNavItems: { id: InstanceTabId; label: string; icon: React.ElementType }[] = selectedInstance
    ? [
        { id: 'status', label: 'Stato & Metriche', icon: Activity },
        { id: 'console', label: 'Console Live', icon: Terminal },
        ...(selectedInstance.template_id === 'minecraft-java'
          ? [{ id: 'loader' as InstanceTabId, label: 'Loader & Versione', icon: Package }]
          : []),
        ...(selectedInstance.template_id === 'horizon-mc' || selectedInstance.category === 'horizon'
          ? [{ id: 'horizon' as InstanceTabId, label: 'Horizon MC (Rust)', icon: Zap }]
          : []),
        ...(selectedInstance.template_id === 'github-webservice' || selectedInstance.category === 'webservice'
          ? [{ id: 'webservice' as InstanceTabId, label: 'GitHub Web Service', icon: Globe }]
          : []),
        { id: 'config', label: 'Configurazione', icon: Settings },
        { id: 'files', label: 'File Manager', icon: Folder },
        { id: 'ftp', label: 'FTP & Cartelle', icon: HardDrive },
        { id: 'mods', label: 'Mod & Plugin', icon: Package },
        { id: 'backups', label: 'Backup', icon: Archive },
        { id: 'scheduler', label: 'Scheduler', icon: CalendarClock },
        { id: 'access', label: 'Accessi (+)', icon: Users },
      ]
    : [];

  const logout = () => {
    setToken(null);
    setUser(null);
  };

  const showRefreshBanner =
    updateStatus && (updateStatus.ui_hot_updated || updateStatus.state === 'ready_refresh');

  const quickResults = useMemo(() => {
    const q = quickQuery.trim().toLowerCase();
    const list: { label: string; sub: string; action: () => void }[] = [];
    for (const inst of instances) {
      if (!q || inst.name.toLowerCase().includes(q) || inst.template_id.toLowerCase().includes(q)) {
        list.push({
          label: inst.name,
          sub: `Istanza (${inst.status}) • :${inst.port}`,
          action: () => openInstance(inst.id, 'status'),
        });
        if (q.includes('cons') || q.includes('term')) {
          list.push({
            label: `${inst.name} → Console Live`,
            sub: 'Apri terminale WebSocket',
            action: () => openInstance(inst.id, 'console'),
          });
        }
        if (q.includes('ftp') || q.includes('file')) {
          list.push({
            label: `${inst.name} → FTP & File Manager`,
            sub: 'Gestisci cartella server, world, mods',
            action: () => openInstance(inst.id, 'ftp'),
          });
        }
      }
    }
    for (const n of navItems) {
      if (!q || n.label.toLowerCase().includes(q)) {
        list.push({
          label: n.label,
          sub: 'Sezione Pannello ADS',
          action: () => {
            setSelectedInstanceId(null);
            setPage(n.id);
            setQuickOpen(false);
            setQuickQuery('');
          },
        });
      }
    }
    return list.slice(0, 10);
  }, [quickQuery, instances]);

  return (
    <div className="app-shell">
      {/* AMP-style Context-Switching Left Sidebar */}
      <aside className="sidebar">
        <div
          className="sidebar-brand"
          style={{ cursor: 'pointer' }}
          onClick={() => {
            setSelectedInstanceId(null);
            setPage('instances');
          }}
        >
          <div className="brand-mark">
            <Zap size={18} />
          </div>
          <div className="brand-info">
            <span className="brand-name">{settings?.panel_name || 'GOLDSMP'}</span>
            <span className="brand-sub">
              v{updateStatus?.active_ui_version || sysInfo?.panel_version || '1.7.0'} • ADS Rust
            </span>
          </div>
        </div>

        <nav className="sidebar-nav">
          {selectedInstance ? (
            <>
              {/* AMP-style Return to Instances button */}
              <div style={{ padding: '2px 4px 10px' }}>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setSelectedInstanceId(null)}
                  style={{ width: '100%', justifyContent: 'flex-start', padding: '9px 12px' }}
                >
                  <ArrowLeft size={15} />
                  <span>Torna alle Istanze</span>
                </button>
              </div>

              {/* Active Instance Context Header */}
              <div
                style={{
                  margin: '0 4px 10px',
                  padding: '10px 12px',
                  borderRadius: 'var(--radius-sm)',
                  background: 'rgba(245, 185, 50, 0.08)',
                  border: '1px solid var(--border-subtle)',
                }}
              >
                <div
                  style={{
                    fontWeight: 700,
                    fontSize: 13.5,
                    color: '#fff4d2',
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {selectedInstance.name}
                </div>
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    marginTop: 4,
                    fontSize: 11,
                    fontFamily: 'var(--font-mono)',
                    color: 'var(--text-secondary)',
                  }}
                >
                  <span>:{selectedInstance.port}</span>
                  <span className={`status-badge status-${selectedInstance.status}`} style={{ padding: '1px 7px', fontSize: 10 }}>
                    <span className="status-dot" />
                    {selectedInstance.status}
                  </span>
                </div>
              </div>

              <div className="nav-section-label">Menu Istanza</div>
              {instanceNavItems.map((item) => {
                const Icon = item.icon;
                const active = instanceTab === item.id;
                return (
                  <button
                    key={item.id}
                    className={`nav-item ${active ? 'active' : ''}`}
                    onClick={() => setInstanceTab(item.id)}
                  >
                    <Icon size={16} />
                    <span>{item.label}</span>
                  </button>
                );
              })}
            </>
          ) : (
            <>
              <div style={{ padding: '2px 4px 10px' }}>
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => setShowCreateModal(true)}
                  style={{
                    width: '100%',
                    padding: '10px 12px',
                    fontSize: 13.5,
                    boxShadow: '0 0 18px rgba(245, 185, 50, 0.35)',
                    border: '1px solid rgba(255, 240, 179, 0.55)',
                  }}
                >
                  <Plus size={16} strokeWidth={2.5} />
                  <span>Crea Nuova Istanza</span>
                </button>
              </div>

              <div className="nav-section-label">Gestione ADS</div>
              {navItems.slice(0, 3).map((item) => {
                const Icon = item.icon;
                const active = page === item.id;
                return (
                  <button
                    key={item.id}
                    className={`nav-item ${active ? 'active' : ''}`}
                    onClick={() => setPage(item.id)}
                  >
                    <Icon size={17} />
                    <span>{item.label}</span>
                  </button>
                );
              })}

              <div className="nav-section-label">Amministrazione</div>
              {navItems.slice(3).map((item) => {
                const Icon = item.icon;
                const active = page === item.id;
                return (
                  <button
                    key={item.id}
                    className={`nav-item ${active ? 'active' : ''}`}
                    onClick={() => setPage(item.id)}
                  >
                    <Icon size={17} />
                    <span>{item.label}</span>
                  </button>
                );
              })}
            </>
          )}
        </nav>

        <div className="sidebar-footer">
          <span
            className="ram-pill"
            onClick={toggleRamUnit}
            style={{ cursor: 'pointer' }}
            title="Clicca per cambiare unità RAM tra MB e GB"
          >
            {sysInfo
              ? `${formatRamValue(sysInfo.panel_memory_mb, ramUnit, ramUnit === 'GB' ? 3 : 1)} RAM`
              : 'Rust Engine'}
          </span>
          <button className="btn btn-secondary btn-sm" onClick={logout} title="Esci">
            <LogOut size={14} />
          </button>
        </div>
      </aside>

      {/* Main Column */}
      <div className="main-column">
        <header className="topbar">
          <div className="topbar-left" style={{ gap: 14 }}>
            <span className="topbar-title">
              {selectedInstance
                ? `${selectedInstance.name}`
                : navItems.find((n) => n.id === page)?.label}
            </span>

            {/* AMP-style Quick Search / Jump bar */}
            <div style={{ position: 'relative' }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setQuickOpen((v) => !v)}
                style={{
                  gap: 8,
                  color: 'var(--text-secondary)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: 12,
                  padding: '5px 12px',
                }}
              >
                <Search size={13} />
                <span>Cerca istanza o menu...</span>
                <span
                  style={{
                    fontSize: 10,
                    padding: '1px 5px',
                    borderRadius: 4,
                    background: 'var(--bg-input)',
                    border: '1px solid var(--border-subtle)',
                  }}
                >
                  Ctrl+K
                </span>
              </button>

              {quickOpen && (
                <div
                  className="card"
                  style={{
                    position: 'absolute',
                    top: '115%',
                    left: 0,
                    width: 340,
                    padding: 10,
                    zIndex: 80,
                    border: '1px solid var(--border-strong)',
                  }}
                >
                  <input
                    autoFocus
                    style={{ width: '100%', marginBottom: 8 }}
                    placeholder="Digita istanza, console, ftp, analytics..."
                    value={quickQuery}
                    onChange={(e) => setQuickQuery(e.target.value)}
                  />
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 4, maxHeight: 260, overflowY: 'auto' }}>
                    {quickResults.map((r, i) => (
                      <button
                        key={i}
                        type="button"
                        className="nav-item"
                        style={{ flexDirection: 'column', alignItems: 'flex-start', gap: 1, padding: '7px 10px' }}
                        onClick={r.action}
                      >
                        <span style={{ fontSize: 13, fontWeight: 600, color: '#fff4d2' }}>{r.label}</span>
                        <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{r.sub}</span>
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className="topbar-right">
            <button
              type="button"
              className="btn btn-primary btn-sm"
              onClick={() => setShowCreateModal(true)}
              style={{
                padding: '6px 14px',
                boxShadow: '0 0 14px rgba(245, 185, 50, 0.35)',
              }}
            >
              <Plus size={14} strokeWidth={2.5} /> Nuova Istanza
            </button>
            <div className="pill-group" title="Switch unità visualizzazione e input RAM (MB / GB)">
              <button
                type="button"
                className={`pill-btn ${ramUnit === 'MB' ? 'active' : ''}`}
                style={{ padding: '4px 9px', fontSize: 11.5 }}
                onClick={() => setRamUnit('MB')}
              >
                MB
              </button>
              <button
                type="button"
                className={`pill-btn ${ramUnit === 'GB' ? 'active' : ''}`}
                style={{ padding: '4px 9px', fontSize: 11.5 }}
                onClick={() => setRamUnit('GB')}
              >
                GB
              </button>
            </div>
            {sysInfo && (
              <div className="host-chip" onClick={toggleRamUnit} style={{ cursor: 'pointer' }} title="Clicca per alternare MB / GB">
                <span>
                  CPU <strong>{sysInfo.cpu_usage_percent.toFixed(0)}%</strong>
                </span>
                <span>
                  RAM{' '}
                  <strong>
                    {ramUnit === 'GB'
                      ? `${(sysInfo.used_memory_mb / 1024).toFixed(1)}/${(sysInfo.total_memory_mb / 1024).toFixed(1)} GB`
                      : `${sysInfo.used_memory_mb.toFixed(0)}/${sysInfo.total_memory_mb.toFixed(0)} MB`}
                  </strong>
                </span>
                <span>
                  Disk{' '}
                  <strong>
                    {sysInfo.used_disk_gb.toFixed(0)}/{sysInfo.total_disk_gb.toFixed(0)}G
                  </strong>
                </span>
              </div>
            )}
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => {
                setSelectedInstanceId(null);
                setPage('settings');
              }}
              title="Stato Aggiornamenti & Notifiche"
            >
              <Bell size={14} color={showRefreshBanner ? 'var(--accent)' : undefined} />
            </button>
            <span style={{ fontSize: 13, color: 'var(--text-secondary)', fontWeight: 500 }}>
              {user.username}
            </span>
          </div>
        </header>

        {showRefreshBanner && (
          <div
            style={{
              background: 'var(--accent-soft)',
              borderBottom: '1px solid var(--accent)',
              padding: '10px 24px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: 12,
              flexWrap: 'wrap',
            }}
          >
            <div style={{ fontSize: 13, color: 'var(--text-primary)' }}>
              <strong>Aggiornamento v{updateStatus.latest_version || updateStatus.active_ui_version} scaricato automaticamente!</strong>{' '}
              <span style={{ color: 'var(--text-secondary)' }}>
                Nessun server è stato interrotto: premi <strong>Aggiorna Pagina</strong> per caricare la nuova versione.
              </span>
            </div>
            <button
              className="btn btn-primary btn-sm"
              onClick={() => window.location.reload()}
            >
              <RefreshCw size={14} /> Aggiorna Pagina (F5)
            </button>
          </div>
        )}

        <main className="content-scroll">
          {selectedInstanceId ? (
            <InstanceDetailPage
              instanceId={selectedInstanceId}
              templates={templates}
              activeTab={instanceTab}
              onTabChange={setInstanceTab}
              onBack={() => setSelectedInstanceId(null)}
              onRefreshInstances={refreshCore}
              onNotify={notify}
            />
          ) : page === 'instances' ? (
            <InstancesPage
              instances={instances}
              sysInfo={sysInfo}
              onRefresh={refreshCore}
              onSelectInstance={(id, tab) => openInstance(id, tab)}
              onOpenCreate={() => setShowCreateModal(true)}
              onNotify={notify}
            />
          ) : page === 'analytics' ? (
            <AnalyticsPage instances={instances} onNotify={notify} />
          ) : page === 'webhooks' ? (
            <WebhooksPage settings={settings} onNotify={notify} />
          ) : page === 'users' ? (
            <UsersPage onNotify={notify} />
          ) : page === 'audit' ? (
            <AuditLogPage />
          ) : (
            <SettingsPage
              settings={settings}
              sysInfo={sysInfo}
              onSaved={(s) => {
                setSettings(s);
                document.documentElement.style.setProperty('--accent', s.accent_color);
              }}
              onNotify={notify}
            />
          )}
        </main>
      </div>

      {showCreateModal && (
        <CreateInstanceModal
          templates={templates}
          onClose={() => setShowCreateModal(false)}
          onCreated={(inst) => {
            setShowCreateModal(false);
            notify(`Istanza "${inst.name}" creata!`);
            refreshCore();
            openInstance(inst.id, 'status');
          }}
        />
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

export default App;
