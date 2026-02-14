#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
LOG_DIR="${ROOT_DIR}/logs"
LOG_FILE="${LOG_DIR}/frontend.log"

cd "${SCRIPT_DIR}"
mkdir -p "${LOG_DIR}"

if [[ ! -f node_modules/vite/dist/node/chunks/dist.js ]]; then
	rm -rf node_modules
	npm ci
fi

pkill -f "vite --host --port 5173" || true
nohup npm run dev -- --host --port 5173 > "${LOG_FILE}" 2>&1 &
