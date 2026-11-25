#!/usr/bin/env bash
set -euo pipefail

echo "=== GOLDSMP Build & Installation Setup ==="
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${GOLDSMP_DATA_DIR:-$SCRIPT_DIR/goldsmp-data}"
PORT="${GOLDSMP_PORT:-8080}"

OAUTH_CLIENT_ID="${GOLDSMP_OAUTH_CLIENT_ID:-}"
OAUTH_CLIENT_SECRET="${GOLDSMP_OAUTH_CLIENT_SECRET:-}"
OAUTH_REDIRECT_URI="${GOLDSMP_OAUTH_REDIRECT_URI:-http://localhost:${PORT}/api/gdrive/callback}"
OAUTH_CLIENT_FILE="${GOLDSMP_OAUTH_CLIENT_FILE:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --oauth-client-id)
      OAUTH_CLIENT_ID="$2"
      shift 2
      ;;
    --oauth-client-secret)
      OAUTH_CLIENT_SECRET="$2"
      shift 2
      ;;
    --oauth-redirect-uri)
      OAUTH_REDIRECT_URI="$2"
      shift 2
      ;;
    --oauth-client-file)
      OAUTH_CLIENT_FILE="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

mkdir -p "$DATA_DIR"

# Provision OAuth client during installation if provided via flags, env vars, or file
if [[ -n "$OAUTH_CLIENT_FILE" && -f "$OAUTH_CLIENT_FILE" ]]; then
  cp "$OAUTH_CLIENT_FILE" "$DATA_DIR/oauth_client.json"
  echo "[OAuth Setup] Copiato file client OAuth in $DATA_DIR/oauth_client.json"
elif [[ -n "$OAUTH_CLIENT_ID" ]]; then
  cat > "$DATA_DIR/oauth_client.json" <<EOF
{
  "web": {
    "client_id": "${OAUTH_CLIENT_ID}",
    "client_secret": "${OAUTH_CLIENT_SECRET}",
    "redirect_uris": ["${OAUTH_REDIRECT_URI}"]
  }
}
EOF
  echo "[OAuth Setup] Client OAuth preconfigurato in $DATA_DIR/oauth_client.json"
elif [[ ! -f "$SCRIPT_DIR/oauth_client.json.example" ]]; then
  cat > "$SCRIPT_DIR/oauth_client.json.example" <<EOF
{
  "web": {
    "client_id": "YOUR_GOOGLE_CLIENT_ID.apps.googleusercontent.com",
    "client_secret": "GOCSPX-YOUR_GOOGLE_CLIENT_SECRET",
    "redirect_uris": ["http://localhost:8080/api/gdrive/callback"]
  }
}
EOF
fi

echo "[1/2] Compilazione Frontend React + TypeScript..."
(cd "$SCRIPT_DIR/frontend" && npm install && npm run build)

echo "[2/2] Compilazione binario nativo Rust (release, LTO)..."
(cd "$SCRIPT_DIR" && cargo build --release)

echo ""
echo "Compilazione & Setup completati!"
echo "Binario pronto in: $SCRIPT_DIR/target/release/goldsmp"
echo "Client OAuth (Installazione): $DATA_DIR/oauth_client.json (oppure GOLDSMP_OAUTH_CLIENT_ID / GOLDSMP_OAUTH_CLIENT_SECRET)"
echo "Avvia con: ./target/release/goldsmp"

