#!/usr/bin/env bash
set -euo pipefail

HOST="${1:-10.21.11.92}"
USER_NAME="${2:-ai}"
MODEL_UID="${3:-qwen2-5-0-5b-instruct}"
ROUNDS="${ROUNDS:-6}"
INTERVAL_SEC="${INTERVAL_SEC:-5}"

BFF_URL="http://${HOST}:18090"

pass() { echo "[PASS] $*"; }
fail() { echo "[FAIL] $*"; }
info() { echo "[INFO] $*"; }

check_http_code() {
  local url="$1"
  curl -sS -o /tmp/nebula_check_body.$$ -w "%{http_code}" "$url" || true
}

info "Target host: ${HOST} (ssh: ${USER_NAME}@${HOST})"
info "Target model: ${MODEL_UID}"

echo
info "1) SSH connectivity"
if ssh -o ConnectTimeout=8 "${USER_NAME}@${HOST}" 'echo ok' >/dev/null 2>&1; then
  pass "SSH reachable"
else
  fail "SSH unreachable"
  exit 1
fi

echo
info "2) Core API availability"
models_code="$(check_http_code "${BFF_URL}/api/v2/models")"
audit_code="$(check_http_code "${BFF_URL}/api/audit-logs")"

if [[ "$models_code" == "200" ]]; then
  pass "GET /api/v2/models => 200"
else
  fail "GET /api/v2/models => ${models_code}"
fi

if [[ "$audit_code" == "200" ]]; then
  pass "GET /api/audit-logs => 200"
else
  fail "GET /api/audit-logs => ${audit_code}"
fi

echo
info "3) Model stability polling (${ROUNDS} rounds x ${INTERVAL_SEC}s)"
all_ok=1
for i in $(seq 1 "$ROUNDS"); do
  ts="$(date '+%H:%M:%S')"
  body="$(curl -sS "${BFF_URL}/api/v2/models" || true)"
  line="$(python3 - "$MODEL_UID" "$body" <<'PY'
import json,sys
uid=sys.argv[1]
body=sys.argv[2]
try:
    arr=json.loads(body)
except Exception:
    print("parse_error")
    raise SystemExit(0)
item=next((x for x in arr if x.get("model_uid")==uid),None)
if not item:
    print("missing")
    raise SystemExit(0)
state=item.get("state")
rep=item.get("replicas") or {}
eps=item.get("endpoints") or []
print(f"state={state} ready={rep.get('ready')}/{rep.get('desired')} endpoints={len(eps)}")
PY
)"
  echo "  [${i}/${ROUNDS}] ${ts} ${line}"
  if [[ "$line" != state=running* ]]; then
    all_ok=0
  fi
  sleep "$INTERVAL_SEC"
done

if [[ "$all_ok" -eq 1 ]]; then
  pass "Model remained running during polling"
else
  fail "Model was not consistently running"
fi

echo
info "4) Router freshness metric exposure"
route_metric_ok=1
if ssh -o ConnectTimeout=8 "${USER_NAME}@${HOST}" "curl -fsS http://127.0.0.1:18081/metrics | grep -q '^nebula_router_route_stale_stats_dropped_total '"; then
  pass "Router metric nebula_router_route_stale_stats_dropped_total is exposed"
else
  route_metric_ok=0
  fail "Router metric nebula_router_route_stale_stats_dropped_total missing"
fi

echo
info "5) Remote runtime config snapshot"
ssh "${USER_NAME}@${HOST}" 'set -e; cd ~/github/nebula; echo "commit=$(git rev-parse --short HEAD)"; echo "XTRACE settings:"; grep -E "^XTRACE_(URL|TOKEN|AUTH_MODE)=" deploy/nebula.env || true' || true

echo
if [[ "$models_code" == "200" && "$audit_code" == "200" && "$all_ok" -eq 1 && "$route_metric_ok" -eq 1 ]]; then
  pass "Overall health check PASSED"
  exit 0
else
  fail "Overall health check FAILED"
  exit 2
fi
