#!/usr/bin/env bash
set -euo pipefail

echo "=== GOLDSMP Build & Setup ==="
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "[1/2] Compilazione Frontend React + TypeScript..."
(cd "$SCRIPT_DIR/frontend" && npm install && npm run build)

echo "[2/2] Compilazione binario nativo Rust (release, LTO)..."
(cd "$SCRIPT_DIR" && cargo build --release)

echo ""
echo "Compilazione completata!"
echo "Binario pronto in: $SCRIPT_DIR/target/release/goldsmp"
echo "Avvia con: ./target/release/goldsmp"
