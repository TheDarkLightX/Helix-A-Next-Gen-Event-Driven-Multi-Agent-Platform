#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v docker >/dev/null 2>&1; then
  echo "[compose] missing required command: docker" >&2
  exit 1
fi

cd "$ROOT_DIR"
echo "[compose] starting Helix API, UI, and Postgres"
echo "[compose] UI/API: http://127.0.0.1:${HELIX_API_PORT:-3000}"
docker compose up --build
