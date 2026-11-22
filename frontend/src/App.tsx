import React, { useEffect, useState } from 'react';
import { api, getToken, setToken } from './api/client';
import { AppTemplate, HostSystemInfo, Instance, PanelSettings, UpdateStatus, User } from './types';
import { LoginView } from './components/LoginView';
import { CreateInstanceModal } from './components/CreateInstanceModal';
import { InstancesPage } from './pages/InstancesPage';
import { InstanceDetailPage } from './pages/InstanceDetailPage';
import { AnalyticsPage } from './pages/AnalyticsPage';
import { AuditLogPage, SettingsPage, UsersPage, WebhooksPage } from './pages/OpsPages';
import {
  BarChart3,
  FileClock,
  LogOut,
  Plus,
  RefreshCw,
  Server,
  Settings,
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
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

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
    { id: 'settings', label: 'Impostazioni', icon: Settings },
  ];

  const logout = () => {
    setToken(null);
    setUser(null);
  };

  const showRefreshBanner =
    updateStatus && (updateStatus.ui_hot_updated || updateStatus.state === 'ready_refresh');

  return (
    <div className="app-shell">
      {/* AMP-style Left Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-brand">
          <div className="brand-mark">
            <Zap size={18} />
          </div>
          <div className="brand-info">
            <span className="brand-name">{settings?.panel_name || 'GOLDSMP'}</span>
            <span className="brand-sub">Rust Native • Open</span>
          </div>
        </div>

        <nav className="sidebar-nav">
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

          <div className="nav-section-label">Gestione</div>
          {navItems.slice(0, 3).map((item) => {
            const Icon = item.icon;
            const active = page === item.id && !selectedInstanceId;
            return (
              <button
                key={item.id}
                className={`nav-item ${active ? 'active' : ''}`}
                onClick={() => {
                  setPage(item.id);
                  setSelectedInstanceId(null);
                }}
              >
                <Icon size={17} />
                <span>{item.label}</span>
              </button>
            );
          })}

          <div className="nav-section-label">Amministrazione</div>
          {navItems.slice(3).map((item) => {
            const Icon = item.icon;
            const active = page === item.id && !selectedInstanceId;
            return (
              <button
                key={item.id}
                className={`nav-item ${active ? 'active' : ''}`}
                onClick={() => {
                  setPage(item.id);
                  setSelectedInstanceId(null);
                }}
              >
                <Icon size={17} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar-footer">
          <span className="ram-pill" title="Consumo RAM effettivo del pannello Rust">
            {sysInfo ? `${sysInfo.panel_memory_mb.toFixed(1)} MB RAM` : 'Rust Engine'}
          </span>
          <button className="btn btn-secondary btn-sm" onClick={logout} title="Esci">
            <LogOut size={14} />
          </button>
        </div>
      </aside>

      {/* Main Column */}
      <div className="main-column">
        <header className="topbar">
          <div className="topbar-left">
            <span className="topbar-title">
              {selectedInstanceId
                ? `Gestione Istanza`
                : navItems.find((n) => n.id === page)?.label}
            </span>
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
            {sysInfo && (
              <div className="host-chip">
                <span>
                  CPU <strong>{sysInfo.cpu_usage_percent.toFixed(0)}%</strong>
                </span>
                <span>
                  RAM{' '}
                  <strong>
                    {(sysInfo.used_memory_mb / 1024).toFixed(1)}/
                    {(sysInfo.total_memory_mb / 1024).toFixed(0)}G
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
              onBack={() => setSelectedInstanceId(null)}
              onRefreshInstances={refreshCore}
              onNotify={notify}
            />
          ) : page === 'instances' ? (
            <InstancesPage
              instances={instances}
              sysInfo={sysInfo}
              onRefresh={refreshCore}
              onSelectInstance={(id) => setSelectedInstanceId(id)}
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
            setSelectedInstanceId(inst.id);
          }}
        />
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

export default App;
