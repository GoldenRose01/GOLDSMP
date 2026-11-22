# GOLDSMP

**G**reatest **O**ptimized **L**ightweight **D**edicated **S**erver **M**anagement **P**anel — Alternativa open-source, ultra-leggera e senza limiti di licenza a CubeCoders AMP, scritta in **Rust** con interfaccia web **React + TypeScript** incorporata in un **singolo binario nativo**.

## Perché GOLDSMP invece di AMP?

| Caratteristica | CubeCoders AMP | GOLDSMP |
|---|---|---|
| Motore Backend | C# / .NET (runtime per ogni istanza) | **Rust nativo** (singolo binario, ~10–15 MB RAM) |
| Limite Istanze | 5 / 15 / 50 (in base alla licenza) | **Illimitato** |
| Analytics & Player Tracking | Solo Advanced / Enterprise ($40+ / $20/mese) | **100% Incluso** (sessioni, playtime, IP, export CSV) |
| Geo-Filtering (Auto-Kick) | Solo Advanced / Enterprise | **100% Incluso** |
| Webhook & Automazioni | Solo Pro+ | **100% Incluso** (Discord embeds + Incoming API) |
| Branding & OIDC SSO | Solo Advanced / Enterprise | **100% Incluso** |
| Esecuzione | Nativo o Docker | **100% Processi Nativi** (zero overhead Docker) |
| Database | Esterno / file multipli | **SQLite WAL integrato** (zero configurazione) |

## Funzionalità Incluse

- **Gestione Istanze Native**: Avvio, arresto controllato (stdin + fallback SIGKILL), riavvio, kill, auto-restart.
- **Template Pronti all'Uso**:
  - **Minecraft**: Java Edition (Paper/Spigot/Fabric/Forge/Vanilla) & Bedrock Edition
  - **SteamCMD**: Palworld, Valheim, Counter-Strike 2, Rust, Terraria/tModLoader
  - **App Generiche**: Node.js / Discord Bot, Python 3, Eseguibili Nativi Custom
- **Console Live WebSocket**: Streaming stdout/stderr in tempo reale con invio comandi stdin e buffer storico.
- **Telemetry & Analytics Completi**:
  - Campionamento CPU, RAM, Disco, Rete e Giocatori ogni 5 secondi su SQLite
  - Grafici storici (15m / 1h / 6h / 24h / 7d) per Host e per singola Istanza
  - Classifica Playtime, Cronologia Sessioni con IP e Paese, Distribuzione Geografica, Export CSV
  - Regole **Geo-Filtering** con kick automatico
- **File Manager & Editor Web**: Navigazione sandboxed, editor testo, creazione cartelle/file, download diretto da URL.
- **Store Mod & Plugin (Modrinth)**: Ricerca e installazione 1-click direttamente in `plugins/` o `mods/`.
- **Backup `.tar.gz`**: Creazione, ripristino ed eliminazione archivi compressi.
- **Scheduler**: Task automatici ad intervallo (backup, riavvii, comandi console).
- **Webhook & Audit Log**: Notifiche Discord/HTTP, Incoming Webhook API, Audit Log completo di ogni azione, Utenti illimitati con RBAC, Branding e OIDC SSO.

## Avvio Rapido

```bash
# 1. Compila il frontend React (verrà incorporato nel binario Rust)
cd frontend && npm install && npm run build && cd ..

# 2. Compila il binario singolo ottimizzato
cargo build --release

# 3. Avvia GOLDSMP
./target/release/goldsmp
```

Apri `http://localhost:8080` — Credenziali predefinite: **`admin` / `admin123`**.

### Variabili d'Ambiente

- `GOLDSMP_PORT` (default `8080`)
- `GOLDSMP_DATA_DIR` (default `./goldsmp-data`)
- `GOLDSMP_JWT_SECRET` (imposta una chiave sicura in produzione)
