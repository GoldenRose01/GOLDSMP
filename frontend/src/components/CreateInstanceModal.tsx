import React, { useEffect, useMemo, useState } from 'react';
import { api } from '../api/client';
import { AppTemplate, Instance } from '../types';
import { getGameArt } from '../utils/gameArtwork';
import { Boxes, Cpu, Gamepad2, Sliders, Sparkles, X, Zap } from 'lucide-react';

interface CreateInstanceModalProps {
  templates: AppTemplate[];
  onClose: () => void;
  onCreated: (inst: Instance) => void;
}

export const CreateInstanceModal: React.FC<CreateInstanceModalProps> = ({
  templates,
  onClose,
  onCreated,
}) => {
  const [catFilter, setCatFilter] = useState<string>('all');
  const [selectedTpl, setSelectedTpl] = useState<AppTemplate | null>(templates[0] || null);
  const [name, setName] = useState(templates[0]?.name || '');
  const [port, setPort] = useState<number>(templates[0]?.default_port || 25565);
  const [maxMem, setMaxMem] = useState<number>(2048);
  const [maxPlayers, setMaxPlayers] = useState<number>(30);
  const [autoStart, setAutoStart] = useState<boolean>(false);
  const [autoRestart, setAutoRestart] = useState<boolean>(true);
  const [executable, setExecutable] = useState<string>(templates[0]?.default_executable || '');
  const [args, setArgs] = useState<string>((templates[0]?.default_args || []).join(' '));
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [activeSettingsCategory, setActiveSettingsCategory] = useState<string>('all');
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState('');

  const filtered = templates.filter((t) => catFilter === 'all' || t.category === catFilter);

  const pickTemplate = (tpl: AppTemplate) => {
    setSelectedTpl(tpl);
    setPort(tpl.default_port);
    setName(tpl.name);
    setExecutable(tpl.default_executable);
    setArgs(tpl.default_args.join(' '));
    const defaults: Record<string, string> = {};
    for (const f of tpl.settings_schema) {
      defaults[f.key] = f.default_value;
    }
    setSettings(defaults);
    setActiveSettingsCategory('all');
  };

  useEffect(() => {
    if (templates.length > 0 && !selectedTpl) {
      pickTemplate(templates[0]);
    }
  }, [templates]);

  const categories = useMemo(() => {
    if (!selectedTpl) return [];
    const set = new Set<string>();
    for (const s of selectedTpl.settings_schema) {
      if (s.category) set.add(s.category);
    }
    return Array.from(set);
  }, [selectedTpl]);

  const visibleSchema = useMemo(() => {
    if (!selectedTpl) return [];
    if (activeSettingsCategory === 'all') return selectedTpl.settings_schema;
    return selectedTpl.settings_schema.filter((s) => s.category === activeSettingsCategory);
  }, [selectedTpl, activeSettingsCategory]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedTpl) return;
    setError('');
    setCreating(true);
    try {
      const created = await api<Instance>('/api/instances', {
        method: 'POST',
        body: JSON.stringify({
          name: name.trim() || selectedTpl.name,
          template_id: selectedTpl.id,
          port: Number(port),
          max_memory_mb: Number(maxMem),
          max_players: Number(maxPlayers),
          auto_start: autoStart,
          auto_restart: autoRestart,
          executable: executable.trim() || selectedTpl.default_executable,
          args: args.trim() ? args.trim().split(/\s+/) : [],
          settings,
        }),
      });
      onCreated(created);
    } catch (err: any) {
      setError(err.message || 'Errore creazione istanza');
    } finally {
      setCreating(false);
    }
  };

  const selectedArt = selectedTpl ? getGameArt(selectedTpl.id, selectedTpl.category) : null;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        style={{
          maxWidth: 1060,
          width: '96%',
          maxHeight: '92vh',
          border: '1px solid rgba(245, 185, 50, 0.5)',
          boxShadow: '0 28px 90px rgba(0, 0, 0, 0.82), 0 0 42px rgba(245, 185, 50, 0.16)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* High-Visibility Iron Man Gold Header */}
        <div
          className="modal-header"
          style={{
            background:
              'linear-gradient(90deg, rgba(245, 185, 50, 0.16) 0%, rgba(185, 28, 28, 0.1) 60%, transparent 100%)',
            padding: '18px 24px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <div className="brand-mark" style={{ width: 40, height: 40 }}>
              <Sparkles size={20} />
            </div>
            <div>
              <h2
                style={{
                  fontFamily: 'var(--font-display)',
                  fontSize: 22,
                  fontWeight: 700,
                  letterSpacing: '0.05em',
                  textTransform: 'uppercase',
                  color: '#fff4d2',
                }}
              >
                Dispiega Nuova Istanza Server
              </h2>
              <p style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>
                Seleziona il motore di gioco, personalizza risorse, porte e opzioni complete prima dell&apos;avvio.
              </p>
            </div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleCreate} style={{ display: 'flex', flexDirection: 'column', flex: 1, minHeight: 0 }}>
          <div className="modal-body" style={{ padding: '20px 24px', gap: 18 }}>
            {/* Category Filter Bar */}
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                {[
                  { id: 'all', label: 'Tutti i Motori' },
                  { id: 'horizon', label: 'Horizon MC (Rust 26.3)' },
                  { id: 'minecraft', label: 'Minecraft Java & Bedrock' },
                  { id: 'steamcmd', label: 'SteamCMD Dedicated' },
                  { id: 'generic', label: 'Node.js / Python / Custom' },
                ].map((c) => (
                  <button
                    type="button"
                    key={c.id}
                    className={`btn btn-sm ${catFilter === c.id ? 'btn-primary' : 'btn-secondary'}`}
                    onClick={() => setCatFilter(c.id)}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
              <span style={{ fontSize: 12, fontFamily: 'var(--font-mono)', color: 'var(--accent-hover)' }}>
                {filtered.length} template disponibili
              </span>
            </div>

            {/* Game Template Cards with Transparent Background Artwork */}
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(290px, 1fr))',
                gap: 12,
                maxHeight: 270,
                overflowY: 'auto',
                paddingRight: 4,
              }}
            >
              {filtered.map((tpl) => {
                const active = selectedTpl?.id === tpl.id;
                const art = getGameArt(tpl.id, tpl.category);
                return (
                  <button
                    type="button"
                    key={tpl.id}
                    onClick={() => pickTemplate(tpl)}
                    style={{
                      position: 'relative',
                      overflow: 'hidden',
                      textAlign: 'left',
                      padding: '14px 16px',
                      borderRadius: 'var(--radius-md)',
                      background: active
                        ? 'linear-gradient(135deg, rgba(245, 185, 50, 0.22), rgba(20, 20, 28, 0.95))'
                        : 'linear-gradient(135deg, hsl(240, 13%, 12%), hsl(240, 14%, 8.5%))',
                      border: active
                        ? `2px solid ${art.accent}`
                        : '1px solid var(--border-subtle)',
                      boxShadow: active ? `0 0 22px rgba(245, 185, 50, 0.22)` : 'none',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 6,
                      minHeight: 112,
                      transition: 'transform 0.15s ease, border-color 0.15s ease',
                    }}
                  >
                    {/* Semi-transparent Game Illustration Watermark */}
                    <div
                      style={{
                        position: 'absolute',
                        inset: 0,
                        backgroundImage: `url("${art.bgSvgDataUri}")`,
                        backgroundRepeat: 'no-repeat',
                        backgroundPosition: 'right -10px center',
                        backgroundSize: '230px auto',
                        opacity: active ? 0.95 : 0.65,
                        pointerEvents: 'none',
                      }}
                    />

                    <div style={{ position: 'relative', zIndex: 1, display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontWeight: 700, fontSize: 14, color: '#fff4d2' }}>
                        {tpl.category === 'horizon' && <Zap size={16} color={art.accent} />}
                        {tpl.category === 'minecraft' && <Boxes size={16} color={art.accent} />}
                        {tpl.category === 'steamcmd' && <Gamepad2 size={16} color={art.accent} />}
                        {tpl.category === 'generic' && <Cpu size={16} color={art.accent} />}
                        <span>{tpl.name}</span>
                      </div>
                      <span
                        style={{
                          fontSize: 10,
                          fontFamily: 'var(--font-mono)',
                          padding: '2px 7px',
                          borderRadius: 99,
                          background: 'rgba(0,0,0,0.45)',
                          border: `1px solid ${art.accent}55`,
                          color: art.accent,
                          fontWeight: 600,
                        }}
                      >
                        :{tpl.default_port}
                      </span>
                    </div>

                    <span
                      style={{
                        position: 'relative',
                        zIndex: 1,
                        fontSize: 12,
                        color: 'var(--text-secondary)',
                        lineHeight: 1.38,
                        maxWidth: '88%',
                      }}
                    >
                      {tpl.description}
                    </span>

                    <div style={{ position: 'relative', zIndex: 1, marginTop: 'auto', paddingTop: 4, display: 'flex', gap: 6, alignItems: 'center' }}>
                      <span style={{ fontSize: 10, fontFamily: 'var(--font-mono)', color: 'var(--text-muted)', textTransform: 'uppercase' }}>
                        {art.badge} • {tpl.settings_schema.length} opzioni configurabili
                      </span>
                    </div>
                  </button>
                );
              })}
            </div>

            {/* Selected Game Hero Banner + Core Settings */}
            {selectedTpl && selectedArt && (
              <div
                className="card"
                style={{
                  position: 'relative',
                  overflow: 'hidden',
                  padding: 18,
                  border: `1px solid ${selectedArt.accent}55`,
                }}
              >
                <div
                  style={{
                    position: 'absolute',
                    inset: 0,
                    backgroundImage: `url("${selectedArt.bgSvgDataUri}")`,
                    backgroundRepeat: 'no-repeat',
                    backgroundPosition: 'right 10px center',
                    backgroundSize: '310px auto',
                    opacity: 0.45,
                    pointerEvents: 'none',
                  }}
                />
                <div style={{ position: 'relative', zIndex: 1 }}>
                  <div className="card-header" style={{ marginBottom: 12 }}>
                    <span className="card-title">
                      Parametri Principali — {selectedTpl.name}
                    </span>
                    <span style={{ fontSize: 11.5, fontFamily: 'var(--font-mono)', color: selectedArt.accent }}>
                      {selectedArt.badge}
                    </span>
                  </div>

                  <div className="grid-4" style={{ marginBottom: 12 }}>
                    <div className="field">
                      <label>Nome Istanza</label>
                      <input
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                        placeholder={selectedTpl.name}
                        required
                      />
                    </div>
                    <div className="field">
                      <label>Porta Principale</label>
                      <input
                        type="number"
                        value={port}
                        onChange={(e) => setPort(Number(e.target.value))}
                        required
                      />
                    </div>
                    <div className="field">
                      <label>Limite RAM (MB)</label>
                      <input
                        type="number"
                        value={maxMem}
                        onChange={(e) => setMaxMem(Number(e.target.value))}
                      />
                    </div>
                    <div className="field">
                      <label>Slot Giocatori Massimi</label>
                      <input
                        type="number"
                        value={maxPlayers}
                        onChange={(e) => setMaxPlayers(Number(e.target.value))}
                      />
                    </div>
                  </div>

                  <div className="grid-2" style={{ marginBottom: 10 }}>
                    <div className="field">
                      <label>Comando / Binario Eseguibile</label>
                      <input value={executable} onChange={(e) => setExecutable(e.target.value)} />
                    </div>
                    <div className="field">
                      <label>Argomenti di Avvio ({'{PORT}'}, {'{MAX_MEMORY}'}, {'{MAX_PLAYERS}'})</label>
                      <input value={args} onChange={(e) => setArgs(e.target.value)} />
                    </div>
                  </div>

                  <div style={{ display: 'flex', gap: 20, flexWrap: 'wrap', paddingTop: 4 }}>
                    <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                      <input
                        type="checkbox"
                        checked={autoRestart}
                        onChange={(e) => setAutoRestart(e.target.checked)}
                      />
                      Auto-Restart immediato in caso di crash
                    </label>
                    <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                      <input
                        type="checkbox"
                        checked={autoStart}
                        onChange={(e) => setAutoStart(e.target.checked)}
                      />
                      Avvia automaticamente all&apos;accensione di GOLDSMP
                    </label>
                  </div>
                </div>
              </div>
            )}

            {/* Complete Categorized Game Options right inside creation wizard */}
            {selectedTpl && selectedTpl.settings_schema.length > 0 && (
              <div className="card" style={{ padding: 18 }}>
                <div className="card-header" style={{ flexWrap: 'wrap', marginBottom: 12 }}>
                  <span className="card-title">
                    <Sliders size={16} color="var(--accent)" /> Opzioni Complete del Server ({selectedTpl.settings_schema.length} parametri)
                  </span>
                  <div className="pill-group" style={{ flexWrap: 'wrap' }}>
                    <button
                      type="button"
                      className={`pill-btn ${activeSettingsCategory === 'all' ? 'active' : ''}`}
                      onClick={() => setActiveSettingsCategory('all')}
                    >
                      Tutte ({selectedTpl.settings_schema.length})
                    </button>
                    {categories.map((cat) => (
                      <button
                        type="button"
                        key={cat}
                        className={`pill-btn ${activeSettingsCategory === cat ? 'active' : ''}`}
                        onClick={() => setActiveSettingsCategory(cat)}
                      >
                        {cat}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="grid-3">
                  {visibleSchema.map((field) => (
                    <div key={field.key} className="field">
                      <label>{field.label}</label>
                      {field.field_type === 'select' && field.options ? (
                        <select
                          value={settings[field.key] ?? field.default_value}
                          onChange={(e) =>
                            setSettings((prev) => ({ ...prev, [field.key]: e.target.value }))
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
                          value={settings[field.key] ?? field.default_value}
                          onChange={(e) =>
                            setSettings((prev) => ({ ...prev, [field.key]: e.target.value }))
                          }
                        >
                          <option value="true">Attivo (true)</option>
                          <option value="false">Disattivo (false)</option>
                        </select>
                      ) : (
                        <input
                          type={field.field_type === 'number' ? 'number' : 'text'}
                          value={settings[field.key] ?? field.default_value}
                          onChange={(e) =>
                            setSettings((prev) => ({ ...prev, [field.key]: e.target.value }))
                          }
                        />
                      )}
                      <span className="field-hint">{field.description}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {error && <div style={{ color: 'var(--danger)', fontSize: 13 }}>{error}</div>}
          </div>

          <div className="modal-footer" style={{ padding: '14px 24px' }}>
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              Annulla
            </button>
            <button
              type="submit"
              className="btn btn-primary"
              style={{ padding: '10px 22px', fontSize: 14 }}
              disabled={creating || !selectedTpl}
            >
              <Sparkles size={15} />
              {creating ? 'Creazione Istanza...' : `Crea Istanza ${name || ''}`}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
