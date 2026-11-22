import React, { useEffect, useRef, useState } from 'react';
import { api, consoleWsUrl, formatBytes, formatDuration } from '../api/client';
import {
  AppTemplate,
  BackupEntry,
  ConsoleLine,
  FileEntry,
  FtpInfo,
  Instance,
  MetricPoint,
  ModrinthHit,
  ScheduledTask,
  User,
} from '../types';
import { Gauge } from '../components/Gauge';
import { SparkChart } from '../components/SparkChart';
import {
  Activity,
  AlertTriangle,
  Archive,
  ArrowLeft,
  CalendarClock,
  Download,
  FileText,
  Folder,
  FolderPlus,
  HardDrive,
  KeyRound,
  Package,
  Play,
  Plus,
  RotateCw,
  Save,
  Search,
  Send,
  Settings,
  Skull,
  Square,
  Terminal,
  Trash2,
  Users,
  X,
  Zap,
} from 'lucide-react';

interface InstanceDetailPageProps {
  instanceId: string;
  templates: AppTemplate[];
  onBack: () => void;
  onRefreshInstances: () => void;
  onNotify: (msg: string) => void;
}

type TabId =
  | 'status'
  | 'console'
  | 'horizon'
  | 'config'
  | 'files'
  | 'ftp'
  | 'access'
  | 'mods'
  | 'backups'
  | 'scheduler';

export const InstanceDetailPage: React.FC<InstanceDetailPageProps> = ({
  instanceId,
  templates,
  onBack,
  onRefreshInstances,
  onNotify,
}) => {
  const [tab, setTab] = useState<TabId>('status');
  const [inst, setInst] = useState<Instance | null>(null);
  const [metrics, setMetrics] = useState<MetricPoint[]>([]);
  const [ftpInfo, setFtpInfo] = useState<FtpInfo | null>(null);
  const [panelUsers, setPanelUsers] = useState<User[]>([]);
  const [horizonInstalling, setHorizonInstalling] = useState(false);

  // Per-instance user creation form
  const [newUsername, setNewUsername] = useState('');
  const [newEmail, setNewEmail] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [newScope, setNewScope] = useState<'this' | 'all'>('this');

  // Console state
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const [cmdInput, setCmdInput] = useState('');
  const wsRef = useRef<WebSocket | null>(null);
  const consoleEndRef = useRef<HTMLDivElement | null>(null);

  // Config state
  const [cfgName, setCfgName] = useState('');
  const [cfgPort, setCfgPort] = useState(25565);
  const [cfgExec, setCfgExec] = useState('');
  const [cfgArgs, setCfgArgs] = useState('');
  const [cfgMaxMem, setCfgMaxMem] = useState(2048);
  const [cfgSettings, setCfgSettings] = useState<Record<string, string>>({});

  // Files state
  const [curPath, setCurPath] = useState('');
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [editingPath, setEditingPath] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');
  const [dlUrl, setDlUrl] = useState('');
  const [dlName, setDlName] = useState('');

  // Mods state
  const [modQuery, setModQuery] = useState('');
  const [modType, setModType] = useState('plugin');
  const [modHits, setModHits] = useState<ModrinthHit[]>([]);
  const [modLoading, setModLoading] = useState(false);

  // Backups state
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [backupName, setBackupName] = useState('');

  // Scheduler state
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [taskName, setTaskName] = useState('');
  const [taskInterval, setTaskInterval] = useState(3600);
  const [taskAction, setTaskAction] = useState<'command' | 'restart' | 'backup'>('backup');
  const [taskPayload, setTaskPayload] = useState('');

  const loadInstance = async () => {
    try {
      const data = await api<Instance>(`/api/instances/${instanceId}`);
      setInst(data);
    } catch {
      onBack();
    }
  };

  const loadMetrics = async () => {
    try {
      const pts = await api<MetricPoint[]>(
        `/api/analytics/metrics?instance_id=${encodeURIComponent(instanceId)}&range=1h`
      );
      setMetrics(pts);
    } catch {}
  };

  useEffect(() => {
    loadInstance();
    loadMetrics();
    const timer = setInterval(() => {
      loadInstance();
      loadMetrics();
    }, 3500);
    return () => clearInterval(timer);
  }, [instanceId]);

  useEffect(() => {
    if (inst && !cfgName) {
      setCfgName(inst.name);
      setCfgPort(inst.port);
      setCfgExec(inst.executable);
      setCfgArgs(inst.args.join(' '));
      setCfgMaxMem(inst.max_memory_mb);
      setCfgSettings(inst.settings || {});
    }
  }, [inst]);

  // Console WebSocket
  useEffect(() => {
    if (tab !== 'console') return;
    setLines([]);
    const ws = new WebSocket(consoleWsUrl(instanceId));
    wsRef.current = ws;
    ws.onmessage = (ev) => {
      try {
        const line: ConsoleLine = JSON.parse(ev.data);
        setLines((prev) => [...prev.slice(-450), line]);
      } catch {}
    };
    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [tab, instanceId]);

  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [lines]);

  // Tab data loaders
  useEffect(() => {
    if (tab === 'files' || tab === 'ftp' || tab === 'horizon') {
      loadFiles(curPath);
      loadFtpInfo();
    }
    if (tab === 'access') loadPanelUsers();
    if (tab === 'backups') loadBackups();
    if (tab === 'scheduler') loadTasks();
    if (tab === 'mods' && modHits.length === 0) searchMods();
  }, [tab, curPath]);

  const loadFtpInfo = async () => {
    try {
      const info = await api<FtpInfo>(`/api/instances/${instanceId}/ftp`);
      setFtpInfo(info);
    } catch {}
  };

  const loadPanelUsers = async () => {
    try {
      const list = await api<User[]>('/api/users');
      setPanelUsers(list);
    } catch {}
  };

  const loadFiles = async (p: string) => {
    try {
      const list = await api<FileEntry[]>(
        `/api/instances/${instanceId}/files?path=${encodeURIComponent(p)}`
      );
      setFiles(list);
    } catch (err: any) {
      onNotify(err.message);
    }
  };

  const loadBackups = async () => {
    const list = await api<BackupEntry[]>(`/api/instances/${instanceId}/backups`);
    setBackups(list);
  };

  const loadTasks = async () => {
    const list = await api<ScheduledTask[]>(`/api/tasks?instance_id=${instanceId}`);
    setTasks(list);
  };

  const searchMods = async () => {
    setModLoading(true);
    try {
      const hits = await api<ModrinthHit[]>(
        `/api/mods/search?q=${encodeURIComponent(modQuery)}&project_type=${modType}`
      );
      setModHits(hits);
    } catch (err: any) {
      onNotify(`Modrinth: ${err.message}`);
    } finally {
      setModLoading(false);
    }
  };

  const powerAction = async (action: 'start' | 'stop' | 'restart' | 'kill') => {
    try {
      await api(`/api/instances/${instanceId}/${action}`, { method: 'POST' });
      onNotify(`Comando ${action.toUpperCase()} eseguito`);
      loadInstance();
      onRefreshInstances();
    } catch (err: any) {
      onNotify(`Errore: ${err.message}`);
    }
  };

  const sendConsoleCmd = async (e: React.FormEvent) => {
    e.preventDefault();
    const cmd = cmdInput.trim();
    if (!cmd) return;
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(cmd);
    } else {
      await api(`/api/instances/${instanceId}/command`, {
        method: 'POST',
        body: JSON.stringify({ command: cmd }),
      });
    }
    setCmdInput('');
  };

  const saveConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await api(`/api/instances/${instanceId}`, {
        method: 'PUT',
        body: JSON.stringify({
          name: cfgName,
          port: Number(cfgPort),
          executable: cfgExec,
          args: cfgArgs.trim() ? cfgArgs.trim().split(/\s+/) : [],
          max_memory_mb: Number(cfgMaxMem),
          settings: cfgSettings,
        }),
      });
      onNotify('Configurazione salvata e sincronizzata su disco');
      loadInstance();
      onRefreshInstances();
    } catch (err: any) {
      onNotify(`Errore: ${err.message}`);
    }
  };

  const openFile = async (entry: FileEntry) => {
    if (entry.is_dir) {
      setCurPath(entry.path);
      setEditingPath(null);
      return;
    }
    try {
      const res = await api<{ path: string; content: string }>(
        `/api/instances/${instanceId}/files/content?path=${encodeURIComponent(entry.path)}`
      );
      setEditingPath(res.path);
      setEditingContent(res.content);
    } catch (err: any) {
      onNotify(err.message);
    }
  };

  const saveFile = async () => {
    if (!editingPath) return;
    await api(`/api/instances/${instanceId}/files/content`, {
      method: 'PUT',
      body: JSON.stringify({ path: editingPath, content: editingContent }),
    });
    onNotify(`File ${editingPath} salvato`);
  };

  // Triple-confirmation delete modal state
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [deleteStep, setDeleteStep] = useState<1 | 2 | 3>(1);
  const [deleteAckChecked, setDeleteAckChecked] = useState(false);
  const [deleteFilesChecked, setDeleteFilesChecked] = useState(false);
  const [deleteConfirmText, setDeleteConfirmText] = useState('');
  const [deleting, setDeleting] = useState(false);

  const openDeleteModal = () => {
    setDeleteStep(1);
    setDeleteAckChecked(false);
    setDeleteFilesChecked(false);
    setDeleteConfirmText('');
    setDeleteModalOpen(true);
  };

  const executeFinalDelete = async () => {
    if (deleteConfirmText.trim() !== 'CONFERMA') return;
    setDeleting(true);
    try {
      await api(
        `/api/instances/${instanceId}?delete_files=${deleteFilesChecked ? 'true' : 'false'}`,
        { method: 'DELETE' }
      );
      setDeleteModalOpen(false);
      onNotify(
        deleteFilesChecked
          ? 'Istanza e file eliminati definitivamente'
          : 'Istanza eliminata dal pannello'
      );
      onRefreshInstances();
      onBack();
    } catch (err: any) {
      onNotify(`Errore eliminazione: ${err.message}`);
    } finally {
      setDeleting(false);
    }
  };

  if (!inst) {
    return <div className="card">Caricamento istanza...</div>;
  }

  const tpl = templates.find((t) => t.id === inst.template_id);
  const isRunning = inst.status === 'running' || inst.status === 'starting';
  const memPct = inst.max_memory_mb ? (inst.memory_mb / inst.max_memory_mb) * 100 : 0;
  const playersPct = inst.max_players ? (inst.active_players.length / inst.max_players) * 100 : 0;

  return (
    <div>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 14 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <button className="btn btn-secondary btn-sm" onClick={onBack}>
            <ArrowLeft size={15} /> Istanze
          </button>
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <h1>{inst.name}</h1>
              <span className={`status-badge status-${inst.status}`}>
                <span className="status-dot" />
                {inst.status}
              </span>
            </div>
            <p style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
              {inst.template_id} • Porta {inst.port} • PID {inst.pid ?? '--'} • Uptime {formatDuration(inst.uptime_secs)}
            </p>
          </div>
        </div>

        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <button className="btn btn-success" disabled={isRunning} onClick={() => powerAction('start')}>
            <Play size={14} /> Avvia
          </button>
          <button className="btn btn-warning" disabled={!isRunning} onClick={() => powerAction('restart')}>
            <RotateCw size={14} /> Riavvia
          </button>
          <button className="btn btn-danger" disabled={!isRunning} onClick={() => powerAction('stop')}>
            <Square size={14} /> Stop
          </button>
          <button className="btn btn-secondary" disabled={!isRunning} onClick={() => powerAction('kill')} title="SIGKILL">
            <Skull size={14} /> Kill
          </button>
          <button className="btn btn-danger" onClick={openDeleteModal} title="Elimina istanza (tripla conferma)">
            <Trash2 size={14} /> Elimina
          </button>
        </div>
      </div>

      {/* AMP-style Sub-navigation */}
      <div className="subtabs">
        {[
          { id: 'status', label: 'Stato & Metriche', icon: Activity },
          { id: 'console', label: 'Console Live', icon: Terminal },
          ...(inst.template_id === 'horizon-mc' || inst.category === 'horizon'
            ? [{ id: 'horizon', label: 'Horizon MC (Rust)', icon: Zap }]
            : []),
          { id: 'config', label: 'Configurazione', icon: Settings },
          { id: 'files', label: 'File Manager', icon: Folder },
          { id: 'ftp', label: 'FTP & Cartelle', icon: HardDrive },
          { id: 'access', label: 'Accessi (+)', icon: Users },
          { id: 'mods', label: 'Mod & Plugin', icon: Package },
          { id: 'backups', label: 'Backup', icon: Archive },
          { id: 'scheduler', label: 'Scheduler', icon: CalendarClock },
        ].map((t) => {
          const Icon = t.icon;
          return (
            <button
              key={t.id}
              className={`subtab ${tab === t.id ? 'active' : ''}`}
              onClick={() => setTab(t.id as TabId)}
            >
              <Icon size={15} /> {t.label}
            </button>
          );
        })}
      </div>

      {/* TAB 1: STATUS */}
      {tab === 'status' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <div className="grid-4">
            <Gauge
              label="Utilizzo CPU"
              value={inst.cpu_percent}
              displayValue={`${inst.cpu_percent.toFixed(1)}%`}
              sublabel="Processo nativo"
            />
            <Gauge
              label="Memoria RAM"
              value={memPct}
              displayValue={`${inst.memory_mb.toFixed(0)} MB`}
              sublabel={`di ${inst.max_memory_mb} MB`}
              color="var(--success)"
            />
            <Gauge
              label="Giocatori Online"
              value={playersPct}
              displayValue={`${inst.active_players.length} / ${inst.max_players}`}
              sublabel="Tracking live"
              color="#38bdf8"
            />
            <Gauge
              label="Spazio Disco"
              value={Math.min(100, inst.disk_mb / 50)}
              displayValue={`${inst.disk_mb.toFixed(1)} MB`}
              sublabel="Directory istanza"
              color="#f59e0b"
            />
          </div>

          <div className="grid-2">
            <SparkChart
              title="Storico CPU Istanza (1h)"
              unit="%"
              data={metrics.map((m) => ({ timestamp: m.timestamp, value: m.cpu_percent }))}
              maxOverride={100}
            />
            <SparkChart
              title="Storico RAM Istanza (1h)"
              unit="MB"
              color="#10b981"
              data={metrics.map((m) => ({ timestamp: m.timestamp, value: m.memory_mb }))}
            />
          </div>

          <div className="card">
            <div className="card-header">
              <span className="card-title">Giocatori Connessi ({inst.active_players.length})</span>
              <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                Rilevamento automatico dai log in tempo reale
              </span>
            </div>
            {inst.active_players.length === 0 ? (
              <p style={{ color: 'var(--text-muted)', fontSize: 13 }}>
                Nessun giocatore connesso al momento.
              </p>
            ) : (
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                {inst.active_players.map((p) => (
                  <span
                    key={p}
                    style={{
                      background: 'var(--bg-surface-elevated)',
                      border: '1px solid var(--border-subtle)',
                      padding: '5px 12px',
                      borderRadius: 99,
                      fontSize: 13,
                      fontWeight: 500,
                    }}
                  >
                    {p}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* TAB 2: CONSOLE */}
      {tab === 'console' && (
        <div className="console-box">
          <div className="console-output">
            {lines.length === 0 && (
              <div style={{ color: 'var(--text-muted)' }}>
                Console connessa via WebSocket. Avvia l'istanza per visualizzare l'output in tempo reale...
              </div>
            )}
            {lines.map((l, idx) => (
              <div key={idx} className="console-line">
                <span className="console-ts">[{l.timestamp}]</span>
                <span className={`console-stream-${l.stream}`}>{l.message}</span>
              </div>
            ))}
            <div ref={consoleEndRef} />
          </div>
          <form className="console-input-bar" onSubmit={sendConsoleCmd}>
            <input
              placeholder={
                isRunning
                  ? 'Invia comando a stdin (es. help, status, say Ciao, simjoin Alex 79.12.45.8)...'
                  : 'Avvia il server per inviare comandi...'
              }
              value={cmdInput}
              onChange={(e) => setCmdInput(e.target.value)}
              disabled={!isRunning}
            />
            <button type="submit" className="btn btn-primary" disabled={!isRunning}>
              <Send size={14} /> Invia
            </button>
          </form>
        </div>
      )}

      {/* TAB 3: CONFIGURATION */}
      {tab === 'config' && (
        <form onSubmit={saveConfig} style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <div className="card">
            <div className="card-header">
              <span className="card-title">Parametri di Avvio & Processo Nativo</span>
              <button type="submit" className="btn btn-primary btn-sm">
                <Save size={14} /> Salva Configurazione
              </button>
            </div>
            <div className="grid-2">
              <div className="field">
                <label>Nome Istanza</label>
                <input value={cfgName} onChange={(e) => setCfgName(e.target.value)} />
              </div>
              <div className="field">
                <label>Porta</label>
                <input type="number" value={cfgPort} onChange={(e) => setCfgPort(Number(e.target.value))} />
              </div>
              <div className="field">
                <label>Eseguibile</label>
                <input value={cfgExec} onChange={(e) => setCfgExec(e.target.value)} />
              </div>
              <div className="field">
                <label>Argomenti (usa {'{PORT}'}, {'{MAX_MEMORY}'}, {'{MAX_PLAYERS}'})</label>
                <input value={cfgArgs} onChange={(e) => setCfgArgs(e.target.value)} />
              </div>
              <div className="field">
                <label>RAM Massima (MB)</label>
                <input type="number" value={cfgMaxMem} onChange={(e) => setCfgMaxMem(Number(e.target.value))} />
              </div>
              <div className="field">
                <label>Directory di Lavoro</label>
                <input value={inst.work_dir} disabled />
              </div>
            </div>
          </div>

          {tpl && tpl.settings_schema.length > 0 && (
            <div className="card">
              <div className="card-header">
                <span className="card-title">Editor Impostazioni Grafico ({tpl.name})</span>
              </div>
              <div className="grid-2">
                {tpl.settings_schema.map((field) => (
                  <div key={field.key} className="field">
                    <label>{field.label}</label>
                    {field.field_type === 'select' && field.options ? (
                      <select
                        value={cfgSettings[field.key] ?? field.default_value}
                        onChange={(e) =>
                          setCfgSettings((s) => ({ ...s, [field.key]: e.target.value }))
                        }
                      >
                        {field.options.map((opt) => (
                          <option key={opt} value={opt}>
                            {opt}
                          </option>
                        ))}
                      </select>
                    ) : field.field_type === 'boolean' ? (
                      <select
                        value={cfgSettings[field.key] ?? field.default_value}
                        onChange={(e) =>
                          setCfgSettings((s) => ({ ...s, [field.key]: e.target.value }))
                        }
                      >
                        <option value="true">Attivo (true)</option>
                        <option value="false">Disattivo (false)</option>
                      </select>
                    ) : (
                      <input
                        type={field.field_type === 'number' ? 'number' : 'text'}
                        value={cfgSettings[field.key] ?? field.default_value}
                        onChange={(e) =>
                          setCfgSettings((s) => ({ ...s, [field.key]: e.target.value }))
                        }
                      />
                    )}
                    <span className="field-hint">{field.description}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="card" style={{ borderColor: 'rgba(239, 68, 68, 0.25)' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <div style={{ fontWeight: 600, color: 'var(--danger)' }}>Zona Pericolosa</div>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>
                  Rimuove questa istanza dal pannello (arresta il processo se attivo).
                </div>
              </div>
              <button type="button" className="btn btn-danger" onClick={openDeleteModal}>
                <Trash2 size={14} /> Elimina Istanza
              </button>
            </div>
          </div>
        </form>
      )}

      {/* TAB HORIZON MC (RUST) DEDICATED MENU */}
      {tab === 'horizon' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <div className="card">
            <div className="card-header" style={{ flexWrap: 'wrap' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <Zap size={18} color="var(--accent)" />
                <span className="card-title">Horizon MC Server (Rust 26.3 Engine) — Gestione Rapida</span>
              </div>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button
                  className="btn btn-primary btn-sm"
                  disabled={horizonInstalling}
                  onClick={async () => {
                    setHorizonInstalling(true);
                    try {
                      const res = await api<{ message: string }>(
                        `/api/instances/${instanceId}/horizon/install`,
                        { method: 'POST' }
                      );
                      onNotify(res.message);
                      loadFiles(curPath);
                    } catch (err: any) {
                      onNotify(err.message);
                    } finally {
                      setHorizonInstalling(false);
                    }
                  }}
                >
                  <Download size={14} />{' '}
                  {horizonInstalling
                    ? 'Verifica/Download in corso...'
                    : 'Scarica / Aggiorna Release Ufficiale Horizon'}
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={async () => {
                    setTab('files');
                    setCurPath('');
                    try {
                      const res = await api<{ path: string; content: string }>(
                        `/api/instances/${instanceId}/files/content?path=server.toml`
                      );
                      setEditingPath(res.path);
                      setEditingContent(res.content);
                    } catch {}
                  }}
                >
                  <FileText size={14} /> Apri server.toml
                </button>
              </div>
            </div>

            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 14 }}>
              <strong>Horizon MC</strong> è la riscrittura nativa in Rust di Minecraft Java Edition 26.3:
              zero JVM, consumo RAM dimezzato, compatibile sia con client Horizon sia con client Vanilla Java 26.3,
              mondi Vanilla e mod <code>.hmod</code>.
            </p>

            <div className="grid-4" style={{ marginBottom: 16 }}>
              {[
                { label: 'Cartella Mondo (world/)', folder: 'world', desc: 'Salvataggi, region e level.dat' },
                { label: 'Mod Native (mods/)', folder: 'mods', desc: 'Mod Horizon .hmod e addon' },
                { label: 'Datapack (datapacks/)', folder: 'datapacks', desc: 'Datapack Vanilla & Horizon' },
                { label: 'Accesso FTP Diretto', folder: '__ftp__', desc: `Porta ${ftpInfo?.port || 2121} (${ftpInfo?.username || 'inst'})` },
              ].map((item) => (
                <button
                  key={item.folder}
                  type="button"
                  className="card"
                  style={{
                    textAlign: 'left',
                    padding: 14,
                    cursor: 'pointer',
                    border: '1px solid var(--border-subtle)',
                  }}
                  onClick={() => {
                    if (item.folder === '__ftp__') {
                      setTab('ftp');
                    } else {
                      setCurPath(item.folder);
                      setTab('files');
                    }
                  }}
                >
                  <div style={{ fontWeight: 700, color: 'var(--accent-hover)', fontSize: 13.5 }}>
                    {item.label}
                  </div>
                  <div style={{ fontSize: 11.5, color: 'var(--text-secondary)', marginTop: 4 }}>
                    {item.desc}
                  </div>
                </button>
              ))}
            </div>

            {tpl && (
              <form onSubmit={saveConfig}>
                <div className="card-header" style={{ marginTop: 8 }}>
                  <span className="card-title">Configurazione Rapida server.toml</span>
                  <button type="submit" className="btn btn-primary btn-sm">
                    <Save size={14} /> Salva in server.toml
                  </button>
                </div>
                <div className="grid-3">
                  {tpl.settings_schema.map((field) => (
                    <div key={field.key} className="field">
                      <label>{field.label}</label>
                      {field.field_type === 'select' && field.options ? (
                        <select
                          value={cfgSettings[field.key] ?? field.default_value}
                          onChange={(e) =>
                            setCfgSettings((s) => ({ ...s, [field.key]: e.target.value }))
                          }
                        >
                          {field.options.map((opt) => (
                            <option key={opt} value={opt}>
                              {opt}
                            </option>
                          ))}
                        </select>
                      ) : field.field_type === 'boolean' ? (
                        <select
                          value={cfgSettings[field.key] ?? field.default_value}
                          onChange={(e) =>
                            setCfgSettings((s) => ({ ...s, [field.key]: e.target.value }))
                          }
                        >
                          <option value="true">Attivo (true)</option>
                          <option value="false">Disattivo (false)</option>
                        </select>
                      ) : (
                        <input
                          type={field.field_type === 'number' ? 'number' : 'text'}
                          value={cfgSettings[field.key] ?? field.default_value}
                          onChange={(e) =>
                            setCfgSettings((s) => ({ ...s, [field.key]: e.target.value }))
                          }
                        />
                      )}
                      <span className="field-hint">{field.description}</span>
                    </div>
                  ))}
                </div>
              </form>
            )}
          </div>
        </div>
      )}

      {/* TAB FTP & CARTELLE ISTANZA */}
      {tab === 'ftp' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <div className="card">
            <div className="card-header" style={{ flexWrap: 'wrap' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <HardDrive size={18} color="var(--accent)" />
                <span className="card-title">Server FTP Nativo dell&apos;Istanza (FileZilla / WinSCP / Cyberduck)</span>
              </div>
              <button
                className="btn btn-secondary btn-sm"
                onClick={async () => {
                  const updated = await api<FtpInfo>(`/api/instances/${instanceId}/ftp/rotate`, {
                    method: 'POST',
                  });
                  setFtpInfo(updated);
                  onNotify('Nuova password FTP generata');
                }}
              >
                <KeyRound size={14} /> Rigenera Password FTP
              </button>
            </div>

            <p style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
              Ogni istanza ha un accesso FTP isolato (chroot nella directory dell&apos;istanza) per caricare e
              gestire direttamente la cartella del server, il <strong>mondo (world/)</strong>, le{' '}
              <strong>mod (mods/)</strong>, i <strong>plugin (plugins/)</strong> e i file di configurazione.
            </p>

            {ftpInfo && (
              <div className="grid-4" style={{ marginBottom: 16, fontFamily: 'var(--font-mono)', fontSize: 13 }}>
                <div className="card" style={{ padding: 14 }}>
                  <div style={{ color: 'var(--text-muted)', fontSize: 11 }}>HOST / PROTOCOLLO</div>
                  <div style={{ fontWeight: 700, marginTop: 4 }}>
                    ftp://{window.location.hostname || ftpInfo.host}
                  </div>
                </div>
                <div className="card" style={{ padding: 14 }}>
                  <div style={{ color: 'var(--text-muted)', fontSize: 11 }}>PORTA FTP</div>
                  <div style={{ fontWeight: 700, color: 'var(--accent-hover)', marginTop: 4 }}>
                    {ftpInfo.port}
                  </div>
                </div>
                <div className="card" style={{ padding: 14 }}>
                  <div style={{ color: 'var(--text-muted)', fontSize: 11 }}>USERNAME FTP</div>
                  <div style={{ fontWeight: 700, marginTop: 4 }}>{ftpInfo.username}</div>
                </div>
                <div className="card" style={{ padding: 14 }}>
                  <div style={{ color: 'var(--text-muted)', fontSize: 11 }}>PASSWORD FTP</div>
                  <div style={{ fontWeight: 700, color: 'var(--arc-cyan)', marginTop: 4 }}>
                    {ftpInfo.password}
                  </div>
                </div>
              </div>
            )}

            {ftpInfo && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                <div style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>
                  URL Rapido FileZilla / Terminale:
                </div>
                <pre
                  style={{
                    background: 'var(--bg-input)',
                    border: '1px solid var(--border-subtle)',
                    padding: 12,
                    borderRadius: 'var(--radius-sm)',
                    fontFamily: 'var(--font-mono)',
                    fontSize: 12.5,
                    color: 'var(--accent-hover)',
                    overflowX: 'auto',
                  }}
                >
                  {`ftp://${ftpInfo.username}:${ftpInfo.password}@${window.location.hostname || ftpInfo.host}:${ftpInfo.port}/`}
                </pre>
              </div>
            )}
          </div>

          <div className="card">
            <div className="card-header">
              <span className="card-title">Accesso Rapido Cartelle Server (Root, Mondo, Mod, Plugin)</span>
            </div>
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
              <button
                className="btn btn-primary btn-sm"
                onClick={() => {
                  setCurPath('');
                  setTab('files');
                }}
              >
                <Folder size={14} /> Cartella Root Server (/)
              </button>
              {['world', 'mods', 'plugins', 'datapacks', 'logs'].map((folder) => (
                <button
                  key={folder}
                  className="btn btn-secondary btn-sm"
                  onClick={async () => {
                    try {
                      await api(`/api/instances/${instanceId}/files/mkdir`, {
                        method: 'POST',
                        body: JSON.stringify({ path: folder }),
                      });
                    } catch {}
                    setCurPath(folder);
                    setTab('files');
                  }}
                >
                  <Folder size={14} /> /{folder}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* TAB ACCESSI & PERMESSI ISTANZA (+) */}
      {tab === 'access' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
          <div className="card">
            <div className="card-header">
              <span className="card-title">
                Crea Nuovo Utente per "{inst.name}" o Intero Server (con Invio Email Automatico)
              </span>
            </div>
            <p style={{ fontSize: 12.5, color: 'var(--text-secondary)', marginBottom: 14 }}>
              Alla creazione viene inviata automaticamente una mail al possessore con oggetto{' '}
              <strong>&quot;Il tuo account per GOLDSMP è attivo&quot;</strong>. Puoi limitare l&apos;utente solo a
              questa istanza oppure dargli accesso all&apos;intero server.
            </p>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <input
                placeholder="Username"
                value={newUsername}
                onChange={(e) => setNewUsername(e.target.value)}
              />
              <input
                placeholder="Email proprietario (riceverà la mail)"
                value={newEmail}
                onChange={(e) => setNewEmail(e.target.value)}
                style={{ minWidth: 230 }}
              />
              <input
                type="password"
                placeholder="Password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
              />
              <select value={newScope} onChange={(e) => setNewScope(e.target.value as any)}>
                <option value="this">Solo questa istanza ({inst.name})</option>
                <option value="all">Intero Server (Tutte le istanze)</option>
              </select>
              <button
                className="btn btn-primary btn-sm"
                onClick={async () => {
                  if (!newUsername.trim() || !newPassword) return;
                  try {
                    const perms =
                      newScope === 'all'
                        ? ['*']
                        : [`instance:${instanceId}`, 'instances.read', 'instances.control', 'files.write'];
                    await api('/api/users', {
                      method: 'POST',
                      body: JSON.stringify({
                        username: newUsername.trim(),
                        email: newEmail.trim(),
                        password: newPassword,
                        role: newScope === 'all' ? 'admin' : 'operator',
                        permissions: perms,
                      }),
                    });
                    setNewUsername('');
                    setNewEmail('');
                    setNewPassword('');
                    onNotify(
                      `Utente creato! Inviata mail "Il tuo account per GOLDSMP è attivo"`
                    );
                    loadPanelUsers();
                  } catch (err: any) {
                    onNotify(err.message);
                  }
                }}
              >
                <Plus size={14} /> Crea & Invia Email
              </button>
            </div>
          </div>

          <div className="card">
            <div className="card-header">
              <span className="card-title">
                Utenti Esistenti — Premi (+) per abilitare utenti di altre istanze anche su "{inst.name}"
              </span>
            </div>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Username</th>
                    <th>Email</th>
                    <th>Ruolo</th>
                    <th>Accesso Attuale</th>
                    <th style={{ textAlign: 'right' }}>Permesso su "{inst.name}"</th>
                  </tr>
                </thead>
                <tbody>
                  {panelUsers.map((u) => {
                    const isGlobal =
                      u.role === 'superadmin' ||
                      u.role === 'admin' ||
                      u.permissions.includes('*');
                    const hasThis =
                      isGlobal || u.permissions.includes(`instance:${instanceId}`);
                    const otherInstances = u.permissions
                      .filter((p) => p.startsWith('instance:') && p !== `instance:${instanceId}`)
                      .map((p) => p.replace('instance:', '').slice(0, 8));

                    return (
                      <tr key={u.id}>
                        <td style={{ fontWeight: 600 }}>{u.username}</td>
                        <td>{u.email}</td>
                        <td>{u.role}</td>
                        <td style={{ fontSize: 12, fontFamily: 'var(--font-mono)' }}>
                          {isGlobal ? (
                            <span className="status-badge status-running">Intero Server (*)</span>
                          ) : hasThis ? (
                            <span className="status-badge status-starting">
                              Abilitato qui{' '}
                              {otherInstances.length > 0 && `(+${otherInstances.length} altre)`}
                            </span>
                          ) : (
                            <span className="status-badge status-stopped">
                              Altre istanze ({otherInstances.join(', ') || 'nessuna'})
                            </span>
                          )}
                        </td>
                        <td style={{ textAlign: 'right' }}>
                          {isGlobal ? (
                            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                              Accesso Globale
                            </span>
                          ) : hasThis ? (
                            <button
                              className="btn btn-danger btn-sm"
                              onClick={async () => {
                                await api(`/api/users/${u.id}/instances/${instanceId}`, {
                                  method: 'DELETE',
                                });
                                onNotify(`Rimosso accesso a ${u.username} per questa istanza`);
                                loadPanelUsers();
                              }}
                            >
                              Rimuovi da questa istanza
                            </button>
                          ) : (
                            <button
                              className="btn btn-primary btn-sm"
                              title="Aggiungi permesso anche per questa istanza"
                              onClick={async () => {
                                await api(`/api/users/${u.id}/instances`, {
                                  method: 'POST',
                                  body: JSON.stringify({ instance_id: instanceId }),
                                });
                                onNotify(`Aggiunto permesso (+) a ${u.username} per "${inst.name}"!`);
                                loadPanelUsers();
                              }}
                            >
                              <Plus size={14} /> Aggiungi a questa istanza
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
      )}

      {/* TAB 4: FILE MANAGER */}
      {tab === 'files' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div className="card">
            <div className="card-header" style={{ flexWrap: 'wrap' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <span className="card-title">/{curPath}</span>
                {curPath && (
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => {
                      const parts = curPath.split('/').filter(Boolean);
                      parts.pop();
                      setCurPath(parts.join('/'));
                    }}
                  >
                    .. Livello superiore
                  </button>
                )}
                {/* Quick jump buttons for Server / World / Mods / Plugins */}
                <div style={{ display: 'flex', gap: 4, marginLeft: 8, flexWrap: 'wrap' }}>
                  <button
                    className={`btn btn-sm ${curPath === '' ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => setCurPath('')}
                  >
                    Root /
                  </button>
                  {['world', 'mods', 'plugins'].map((qf) => (
                    <button
                      key={qf}
                      className={`btn btn-sm ${curPath === qf ? 'btn-primary' : 'btn-secondary'}`}
                      onClick={async () => {
                        try {
                          await api(`/api/instances/${instanceId}/files/mkdir`, {
                            method: 'POST',
                            body: JSON.stringify({ path: qf }),
                          });
                        } catch {}
                        setCurPath(qf);
                      }}
                    >
                      /{qf}
                    </button>
                  ))}
                  <button
                    className="btn btn-secondary btn-sm"
                    onClick={() => setTab('ftp')}
                    title="Credenziali FTP per FileZilla / WinSCP"
                  >
                    <HardDrive size={13} /> Credenziali FTP
                  </button>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={async () => {
                    const name = prompt('Nome nuova cartella:');
                    if (!name) return;
                    const p = curPath ? `${curPath}/${name}` : name;
                    await api(`/api/instances/${instanceId}/files/mkdir`, {
                      method: 'POST',
                      body: JSON.stringify({ path: p }),
                    });
                    loadFiles(curPath);
                  }}
                >
                  <FolderPlus size={14} /> Nuova Cartella
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  onClick={() => {
                    const name = prompt('Nome nuovo file (es. config.yml):');
                    if (!name) return;
                    const p = curPath ? `${curPath}/${name}` : name;
                    setEditingPath(p);
                    setEditingContent('');
                  }}
                >
                  <FileText size={14} /> Nuovo File
                </button>
              </div>
            </div>

            {/* Download directly from URL (AMP feature!) */}
            <div style={{ display: 'flex', gap: 8, marginBottom: 14, flexWrap: 'wrap' }}>
              <input
                style={{ flex: 2, minWidth: 200 }}
                placeholder="Scarica file da URL diretto (es. PaperMC jar, mod, script)..."
                value={dlUrl}
                onChange={(e) => setDlUrl(e.target.value)}
              />
              <input
                style={{ flex: 1, minWidth: 140 }}
                placeholder="Nome file (es. server.jar)"
                value={dlName}
                onChange={(e) => setDlName(e.target.value)}
              />
              <button
                className="btn btn-primary btn-sm"
                onClick={async () => {
                  if (!dlUrl || !dlName) return;
                  const dest = curPath ? `${curPath}/${dlName}` : dlName;
                  try {
                    await api(`/api/instances/${instanceId}/files/download-url`, {
                      method: 'POST',
                      body: JSON.stringify({ url: dlUrl, dest_path: dest }),
                    });
                    onNotify(`Scaricato ${dest}`);
                    setDlUrl('');
                    setDlName('');
                    loadFiles(curPath);
                  } catch (err: any) {
                    onNotify(err.message);
                  }
                }}
              >
                <Download size={14} /> Scarica URL
              </button>
            </div>

            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Nome</th>
                    <th>Dimensione</th>
                    <th>Modificato</th>
                    <th style={{ textAlign: 'right' }}>Azioni</th>
                  </tr>
                </thead>
                <tbody>
                  {files.map((f) => (
                    <tr key={f.path}>
                      <td>
                        <button
                          onClick={() => openFile(f)}
                          style={{
                            display: 'inline-flex',
                            alignItems: 'center',
                            gap: 8,
                            color: f.is_dir ? 'var(--accent)' : 'var(--text-primary)',
                            fontWeight: f.is_dir ? 600 : 400,
                          }}
                        >
                          {f.is_dir ? <Folder size={15} /> : <FileText size={15} />}
                          {f.name}
                        </button>
                      </td>
                      <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                        {f.is_dir ? '--' : formatBytes(f.size)}
                      </td>
                      <td style={{ color: 'var(--text-secondary)', fontSize: 12 }}>
                        {f.modified ? new Date(f.modified).toLocaleString() : '--'}
                      </td>
                      <td style={{ textAlign: 'right' }}>
                        <button
                          className="btn btn-danger btn-sm"
                          onClick={async () => {
                            if (!confirm(`Eliminare ${f.name}?`)) return;
                            await api(
                              `/api/instances/${instanceId}/files?path=${encodeURIComponent(f.path)}`,
                              { method: 'DELETE' }
                            );
                            loadFiles(curPath);
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

          {editingPath && (
            <div className="card">
              <div className="card-header">
                <span className="card-title" style={{ fontFamily: 'var(--font-mono)' }}>
                  Editor: {editingPath}
                </span>
                <div style={{ display: 'flex', gap: 8 }}>
                  <button className="btn btn-secondary btn-sm" onClick={() => setEditingPath(null)}>
                    Chiudi
                  </button>
                  <button className="btn btn-primary btn-sm" onClick={saveFile}>
                    <Save size={14} /> Salva File
                  </button>
                </div>
              </div>
              <textarea
                className="code-editor"
                value={editingContent}
                onChange={(e) => setEditingContent(e.target.value)}
              />
            </div>
          )}
        </div>
      )}

      {/* TAB 5: MODRINTH STORE */}
      {tab === 'mods' && (
        <div className="card">
          <div className="card-header" style={{ flexWrap: 'wrap' }}>
            <span className="card-title">Store Mod & Plugin (Modrinth 1-Click)</span>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <select value={modType} onChange={(e) => setModType(e.target.value)}>
                <option value="plugin">Plugin (Paper/Spigot)</option>
                <option value="mod">Mod (Fabric/Forge)</option>
                <option value="datapack">Datapack</option>
              </select>
              <input
                placeholder="Cerca su Modrinth (es. Essentials, LuckPerms, Lithium)..."
                value={modQuery}
                onChange={(e) => setModQuery(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && searchMods()}
                style={{ minWidth: 260 }}
              />
              <button className="btn btn-primary btn-sm" onClick={searchMods} disabled={modLoading}>
                <Search size={14} /> {modLoading ? 'Ricerca...' : 'Cerca'}
              </button>
            </div>
          </div>

          <div className="grid-2">
            {modHits.map((m) => (
              <div
                key={m.project_id}
                style={{
                  display: 'flex',
                  gap: 12,
                  padding: 12,
                  borderRadius: 'var(--radius-sm)',
                  background: 'var(--bg-input)',
                  border: '1px solid var(--border-subtle)',
                  alignItems: 'center',
                }}
              >
                {m.icon_url ? (
                  <img
                    src={m.icon_url}
                    alt={m.title}
                    style={{ width: 44, height: 44, borderRadius: 8, objectFit: 'cover' }}
                  />
                ) : (
                  <div className="instance-icon-box">
                    <Package size={18} />
                  </div>
                )}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, fontSize: 13.5 }}>{m.title}</div>
                  <div
                    style={{
                      fontSize: 12,
                      color: 'var(--text-secondary)',
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                    }}
                  >
                    {m.description}
                  </div>
                  <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>
                    by {m.author} • {m.downloads.toLocaleString()} download
                  </div>
                </div>
                <button
                  className="btn btn-primary btn-sm"
                  onClick={async () => {
                    try {
                      const res = await api<{ installed: string; folder: string }>(
                        `/api/instances/${instanceId}/mods/install`,
                        {
                          method: 'POST',
                          body: JSON.stringify({
                            project_id: m.project_id,
                            folder: modType === 'mod' ? 'mods' : 'plugins',
                          }),
                        }
                      );
                      onNotify(`Installato ${res.installed} in ${res.folder}/`);
                    } catch (err: any) {
                      onNotify(`Errore installazione: ${err.message}`);
                    }
                  }}
                >
                  <Download size={13} /> Installa
                </button>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* TAB 6: BACKUPS */}
      {tab === 'backups' && (
        <div className="card">
          <div className="card-header" style={{ flexWrap: 'wrap' }}>
            <span className="card-title">Backup Istanza (.tar.gz)</span>
            <div style={{ display: 'flex', gap: 8 }}>
              <input
                placeholder="Nome backup (opzionale)..."
                value={backupName}
                onChange={(e) => setBackupName(e.target.value)}
              />
              <button
                className="btn btn-primary btn-sm"
                onClick={async () => {
                  try {
                    await api(`/api/instances/${instanceId}/backups`, {
                      method: 'POST',
                      body: JSON.stringify({ name: backupName, notes: 'Backup manuale' }),
                    });
                    setBackupName('');
                    onNotify('Backup creato con successo');
                    loadBackups();
                  } catch (err: any) {
                    onNotify(err.message);
                  }
                }}
              >
                <Archive size={14} /> Crea Backup Ora
              </button>
            </div>
          </div>

          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Nome</th>
                  <th>Archivio</th>
                  <th>Dimensione</th>
                  <th>Creato il</th>
                  <th style={{ textAlign: 'right' }}>Azioni</th>
                </tr>
              </thead>
              <tbody>
                {backups.map((b) => (
                  <tr key={b.id}>
                    <td style={{ fontWeight: 500 }}>{b.name}</td>
                    <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{b.filename}</td>
                    <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                      {formatBytes(b.size_bytes)}
                    </td>
                    <td style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                      {new Date(b.created_at).toLocaleString()}
                    </td>
                    <td style={{ textAlign: 'right', display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                      <button
                        className="btn btn-warning btn-sm"
                        onClick={async () => {
                          if (!confirm(`Ripristinare ${b.name}? I file correnti verranno sovrascritti.`))
                            return;
                          await api(`/api/instances/${instanceId}/backups/${b.id}/restore`, {
                            method: 'POST',
                          });
                          onNotify('Backup ripristinato');
                        }}
                      >
                        Ripristina
                      </button>
                      <button
                        className="btn btn-danger btn-sm"
                        onClick={async () => {
                          await api(`/api/instances/${instanceId}/backups/${b.id}`, {
                            method: 'DELETE',
                          });
                          loadBackups();
                        }}
                      >
                        <Trash2 size={13} />
                      </button>
                    </td>
                  </tr>
                ))}
                {backups.length === 0 && (
                  <tr>
                    <td colSpan={5} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                      Nessun backup presente per questa istanza.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* TAB 7: INSTANCE SCHEDULER */}
      {tab === 'scheduler' && (
        <div className="card">
          <div className="card-header">
            <span className="card-title">Automazioni & Task Programmati</span>
          </div>
          <div style={{ display: 'flex', gap: 8, marginBottom: 16, flexWrap: 'wrap' }}>
            <input
              placeholder="Nome task (es. Backup Notturno)"
              value={taskName}
              onChange={(e) => setTaskName(e.target.value)}
            />
            <select value={taskAction} onChange={(e) => setTaskAction(e.target.value as any)}>
              <option value="backup">Crea Backup</option>
              <option value="restart">Riavvia Server</option>
              <option value="command">Invia Comando Console</option>
            </select>
            <input
              type="number"
              style={{ width: 130 }}
              value={taskInterval}
              onChange={(e) => setTaskInterval(Number(e.target.value))}
              title="Intervallo in secondi"
            />
            {taskAction === 'command' && (
              <input
                placeholder="Comando (es. say Salvataggio...)"
                value={taskPayload}
                onChange={(e) => setTaskPayload(e.target.value)}
              />
            )}
            <button
              className="btn btn-primary btn-sm"
              onClick={async () => {
                if (!taskName.trim()) return;
                await api('/api/tasks', {
                  method: 'POST',
                  body: JSON.stringify({
                    instance_id: instanceId,
                    name: taskName,
                    trigger_type: 'interval',
                    interval_secs: Number(taskInterval),
                    action_type: taskAction,
                    action_payload: taskPayload,
                  }),
                });
                setTaskName('');
                setTaskPayload('');
                onNotify('Task schedulato creato');
                loadTasks();
              }}
            >
              Aggiungi Task
            </button>
          </div>

          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Nome</th>
                  <th>Azione</th>
                  <th>Intervallo</th>
                  <th>Esecuzioni</th>
                  <th style={{ textAlign: 'right' }}>Azioni</th>
                </tr>
              </thead>
              <tbody>
                {tasks.map((t) => (
                  <tr key={t.id}>
                    <td style={{ fontWeight: 500 }}>{t.name}</td>
                    <td>{t.action_type} {t.action_payload && `(${t.action_payload})`}</td>
                    <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                      Ogni {formatDuration(t.interval_secs || 0)}
                    </td>
                    <td>{t.run_count}</td>
                    <td style={{ textAlign: 'right', display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={async () => {
                          await api(`/api/tasks/${t.id}/run`, { method: 'POST' });
                          onNotify('Task eseguito');
                          loadTasks();
                        }}
                      >
                        Esegui Ora
                      </button>
                      <button
                        className="btn btn-danger btn-sm"
                        onClick={async () => {
                          await api(`/api/tasks/${t.id}`, { method: 'DELETE' });
                          loadTasks();
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
      )}
      {/* TRIPLE CONFIRMATION DELETE MODAL */}
      {deleteModalOpen && (
        <div className="modal-backdrop" onClick={() => !deleting && setDeleteModalOpen(false)}>
          <div
            className="modal"
            style={{ maxWidth: 520, borderColor: 'rgba(239, 68, 68, 0.45)' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="modal-header">
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <AlertTriangle size={20} color="var(--danger)" />
                <div>
                  <h2 style={{ fontSize: 16, fontWeight: 700, color: 'var(--danger)' }}>
                    Eliminazione Istanza — Conferma {deleteStep} di 3
                  </h2>
                  <p style={{ fontSize: 12, color: 'var(--text-secondary)' }}>
                    Istanza: <strong>{inst.name}</strong> ({inst.template_id})
                  </p>
                </div>
              </div>
              <button
                className="btn btn-secondary btn-sm"
                disabled={deleting}
                onClick={() => setDeleteModalOpen(false)}
              >
                <X size={15} />
              </button>
            </div>

            <div className="modal-body">
              {/* Step progress bar */}
              <div style={{ display: 'flex', gap: 6 }}>
                {[1, 2, 3].map((step) => (
                  <div
                    key={step}
                    style={{
                      flex: 1,
                      height: 4,
                      borderRadius: 99,
                      background: step <= deleteStep ? 'var(--danger)' : 'var(--bg-input)',
                    }}
                  />
                ))}
              </div>

              {deleteStep === 1 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  <p style={{ fontSize: 13.5 }}>
                    Stai per avviare la procedura di eliminazione dell&apos;istanza{' '}
                    <strong>{inst.name}</strong>.
                  </p>
                  <div
                    style={{
                      background: 'var(--bg-input)',
                      border: '1px solid var(--border-subtle)',
                      borderRadius: 'var(--radius-sm)',
                      padding: 12,
                      fontFamily: 'var(--font-mono)',
                      fontSize: 12,
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 4,
                    }}
                  >
                    <div>Template: {inst.template_id}</div>
                    <div>Porta: {inst.port}</div>
                    <div>Stato attuale: {inst.status}</div>
                    <div>Directory: {inst.work_dir}</div>
                  </div>
                  <p style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>
                    Se il processo è attivo, verrà terminato immediatamente prima della rimozione.
                  </p>
                </div>
              )}

              {deleteStep === 2 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                  <div
                    style={{
                      background: 'var(--danger-soft)',
                      border: '1px solid rgba(239, 68, 68, 0.3)',
                      borderRadius: 'var(--radius-sm)',
                      padding: 12,
                      fontSize: 13,
                      color: 'var(--text-primary)',
                    }}
                  >
                    <strong>Attenzione:</strong> questa operazione rimuoverà definitivamente
                    l&apos;istanza e le sue configurazioni dal pannello GOLDSMP.
                  </div>

                  <label style={{ display: 'flex', alignItems: 'flex-start', gap: 10, fontSize: 13 }}>
                    <input
                      type="checkbox"
                      checked={deleteAckChecked}
                      onChange={(e) => setDeleteAckChecked(e.target.checked)}
                      style={{ marginTop: 3 }}
                    />
                    <span>
                      Confermo di aver capito che l&apos;eliminazione di <strong>{inst.name}</strong> è
                      irreversibile.
                    </span>
                  </label>

                  <label
                    style={{
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: 10,
                      fontSize: 13,
                      color: 'var(--text-secondary)',
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={deleteFilesChecked}
                      onChange={(e) => setDeleteFilesChecked(e.target.checked)}
                      style={{ marginTop: 3 }}
                    />
                    <span>
                      Elimina anche tutti i file e i salvataggi nella cartella su disco (
                      <code>{inst.work_dir}</code>).
                    </span>
                  </label>
                </div>
              )}

              {deleteStep === 3 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                  <p style={{ fontSize: 13.5 }}>
                    Ultima verifica di sicurezza: scrivi esattamente{' '}
                    <strong style={{ color: 'var(--danger)', fontFamily: 'var(--font-mono)' }}>
                      CONFERMA
                    </strong>{' '}
                    nel campo qui sotto per eliminare definitivamente <strong>{inst.name}</strong>.
                  </p>
                  <div className="field">
                    <label>Digita CONFERMA (tutto maiuscolo)</label>
                    <input
                      autoFocus
                      placeholder="CONFERMA"
                      value={deleteConfirmText}
                      onChange={(e) => setDeleteConfirmText(e.target.value)}
                      style={{ fontFamily: 'var(--font-mono)', letterSpacing: '0.06em' }}
                    />
                  </div>
                </div>
              )}
            </div>

            <div className="modal-footer">
              <button
                type="button"
                className="btn btn-secondary"
                disabled={deleting}
                onClick={() => setDeleteModalOpen(false)}
              >
                Annulla
              </button>

              {deleteStep === 1 && (
                <button
                  type="button"
                  className="btn btn-danger"
                  onClick={() => setDeleteStep(2)}
                >
                  Conferma Passo 1/3
                </button>
              )}

              {deleteStep === 2 && (
                <button
                  type="button"
                  className="btn btn-danger"
                  disabled={!deleteAckChecked}
                  onClick={() => setDeleteStep(3)}
                >
                  Conferma Passo 2/3
                </button>
              )}

              {deleteStep === 3 && (
                <button
                  type="button"
                  className="btn btn-danger"
                  disabled={deleteConfirmText.trim() !== 'CONFERMA' || deleting}
                  onClick={executeFinalDelete}
                >
                  <Trash2 size={14} />{' '}
                  {deleting ? 'Eliminazione...' : 'Elimina Definitivamente (3/3)'}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
