#!/bin/bash

set -euo pipefail

stop_by_pattern() {
	local label="$1"
	local pattern="$2"

	echo "Stopping ${label}..."

	# Only target processes owned by current user to avoid noisy permission errors.
	local pids
	pids="$(pgrep -u "$(id -u)" -f "$pattern" || true)"

	if [ -z "$pids" ]; then
		echo "${label} not running"
		return
	fi

	# shellcheck disable=SC2086
	kill $pids >/dev/null 2>&1 || true
}

echo "Stopping Nebula Service Stack..."

stop_by_pattern "Gateway" "nebula-gateway"
stop_by_pattern "BFF" "nebula-bff"
stop_by_pattern "Scheduler" "nebula-scheduler"
stop_by_pattern "Router" "nebula-router"
stop_by_pattern "Node Daemon" "nebula-node"
stop_by_pattern "vLLM Engine" "vllm"

# Optional: Stop Etcd? Usually we might want to keep it running, but for dev simplicity:
# echo "Stopping Etcd..."
# pkill -x "etcd" || echo "Etcd not running"

echo "Cleanup complete."
