import React, { useState } from 'react';
import { api } from '../api/client';
import { AppTemplate, Instance } from '../types';
import { Boxes, Cpu, Gamepad2, X } from 'lucide-react';

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
  const [name, setName] = useState('');
  const [port, setPort] = useState<number>(templates[0]?.default_port || 25565);
  const [maxMem, setMaxMem] = useState<number>(2048);
  const [maxPlayers, setMaxPlayers] = useState<number>(20);
  const [autoRestart, setAutoRestart] = useState<boolean>(true);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState('');

  const filtered = templates.filter((t) => catFilter === 'all' || t.category === catFilter);

  const pickTemplate = (tpl: AppTemplate) => {
    setSelectedTpl(tpl);
    setPort(tpl.default_port);
    if (!name) setName(tpl.name);
  };

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
          auto_restart: autoRestart,
        }),
      });
      onCreated(created);
    } catch (err: any) {
      setError(err.message || 'Errore creazione istanza');
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div>
            <h2 style={{ fontSize: 17, fontWeight: 700 }}>Crea Nuova Istanza</h2>
            <p style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>
              Scegli un template nativo: zero container, zero limiti di istanze.
            </p>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleCreate}>
          <div className="modal-body">
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              {[
                { id: 'all', label: 'Tutti' },
                { id: 'horizon', label: 'Horizon MC (Rust)' },
                { id: 'minecraft', label: 'Minecraft' },
                { id: 'steamcmd', label: 'SteamCMD' },
                { id: 'generic', label: 'App Generiche' },
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

            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
                gap: 10,
                maxHeight: 220,
                overflowY: 'auto',
                paddingRight: 4,
              }}
            >
              {filtered.map((tpl) => {
                const active = selectedTpl?.id === tpl.id;
                return (
                  <button
                    type="button"
                    key={tpl.id}
                    onClick={() => pickTemplate(tpl)}
                    style={{
                      textAlign: 'left',
                      padding: 12,
                      borderRadius: 'var(--radius-sm)',
                      background: active ? 'var(--accent-soft)' : 'var(--bg-input)',
                      border: `1px solid ${active ? 'var(--accent)' : 'var(--border-subtle)'}`,
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 4,
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontWeight: 600, fontSize: 13 }}>
                      {tpl.category === 'minecraft' && <Boxes size={15} color="var(--accent)" />}
                      {tpl.category === 'steamcmd' && <Gamepad2 size={15} color="var(--accent)" />}
                      {tpl.category === 'generic' && <Cpu size={15} color="var(--accent)" />}
                      <span>{tpl.name}</span>
                    </div>
                    <span style={{ fontSize: 11.5, color: 'var(--text-secondary)', lineHeight: 1.35 }}>
                      {tpl.description}
                    </span>
                  </button>
                );
              })}
            </div>

            <div className="grid-2">
              <div className="field">
                <label>Nome Istanza</label>
                <input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={selectedTpl?.name || 'Es. Survival SMP'}
                  required
                />
              </div>
              <div className="field">
                <label>Porta di Rete</label>
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
                <label>Max Giocatori</label>
                <input
                  type="number"
                  value={maxPlayers}
                  onChange={(e) => setMaxPlayers(Number(e.target.value))}
                />
              </div>
            </div>

            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: 'var(--text-secondary)' }}>
              <input
                type="checkbox"
                checked={autoRestart}
                onChange={(e) => setAutoRestart(e.target.checked)}
              />
              Riavvio automatico in caso di crash
            </label>

            {error && (
              <div style={{ color: 'var(--danger)', fontSize: 12.5 }}>{error}</div>
            )}
          </div>

          <div className="modal-footer">
            <button type="button" className="btn btn-secondary" onClick={onClose}>
              Annulla
            </button>
            <button type="submit" className="btn btn-primary" disabled={creating || !selectedTpl}>
              {creating ? 'Creazione...' : 'Crea Istanza'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
