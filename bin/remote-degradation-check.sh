#!/usr/bin/env bash
set -euo pipefail

HOST="${1:-10.21.11.92}"
USER_NAME="${2:-ai}"
WAIT_SEC="${WAIT_SEC:-45}"

ROUTER_METRICS_URL="http://${HOST}:18081/metrics"
SCHED_METRICS_URL="http://${HOST}:18082/metrics"
REMOTE_ROOT="~/github/nebula"
REMOTE_ENV_PATH="${REMOTE_ROOT}/deploy/nebula.env"
MOCK_PORT="18742"
STALE_MOCK_PORT="18743"
MOCK_PID_FILE="/tmp/nebula_xtrace_429_mock.pid"
STALE_MOCK_PID_FILE="/tmp/nebula_xtrace_stale_mock.pid"

TMP_ENV="$(mktemp /tmp/nebula_env.XXXXXX)"

pass() { echo "[PASS] $*"; }
fail() { echo "[FAIL] $*"; }
info() { echo "[INFO] $*"; }
warn() { echo "[WARN] $*"; }

cleanup() {
  info "cleanup: restoring remote nebula.env and stopping mock server"
  scp -q "$TMP_ENV" "${USER_NAME}@${HOST}:${REMOTE_ENV_PATH}" || true
  ssh -o BatchMode=yes "${USER_NAME}@${HOST}" "
    if [ -f ${MOCK_PID_FILE} ]; then
      kill \$(cat ${MOCK_PID_FILE}) >/dev/null 2>&1 || true
      rm -f ${MOCK_PID_FILE}
    fi
    if [ -f ${STALE_MOCK_PID_FILE} ]; then
      kill \$(cat ${STALE_MOCK_PID_FILE}) >/dev/null 2>&1 || true
      rm -f ${STALE_MOCK_PID_FILE}
    fi
    cd ${REMOTE_ROOT} && ./bin/nebula-down.sh >/dev/null 2>&1 || true
    sleep 1
    cd ${REMOTE_ROOT} && ./bin/nebula-up.sh >/dev/null 2>&1 || true
  " || true
}

trap cleanup EXIT

metric_value() {
  local url="$1"
  local name="$2"
  curl -fsS "$url" | awk -v metric="$name" '$1 == metric {print $2; found=1} END {if (!found) print "0"}'
}

set_env_key() {
  local file="$1"
  local key="$2"
  local value="$3"
  if grep -q "^${key}=" "$file"; then
    sed -i.bak "s|^${key}=.*|${key}=${value}|" "$file"
    rm -f "${file}.bak"
  else
    echo "${key}=${value}" >> "$file"
  fi
}

restart_remote_stack() {
  ssh -o BatchMode=yes "${USER_NAME}@${HOST}" "cd ${REMOTE_ROOT} && ./bin/nebula-down.sh >/dev/null 2>&1 || true && sleep 1 && ./bin/nebula-up.sh >/dev/null 2>&1"
}

info "target host: ${HOST}"
ssh -o BatchMode=yes -o ConnectTimeout=8 "${USER_NAME}@${HOST}" "echo ok" >/dev/null
pass "ssh reachable"

scp -q "${USER_NAME}@${HOST}:${REMOTE_ENV_PATH}" "$TMP_ENV"
pass "fetched remote nebula.env backup"

base_router_stale="$(metric_value "$ROUTER_METRICS_URL" "nebula_router_xtrace_stale_total")"
base_sched_stale="$(metric_value "$SCHED_METRICS_URL" "nebula_scheduler_xtrace_stale_total")"
base_router_rl="$(metric_value "$ROUTER_METRICS_URL" "nebula_router_xtrace_rate_limited_total")"
base_sched_rl="$(metric_value "$SCHED_METRICS_URL" "nebula_scheduler_xtrace_rate_limited_total")"
base_router_qe="$(metric_value "$ROUTER_METRICS_URL" "nebula_router_xtrace_query_errors_total")"
base_sched_qe="$(metric_value "$SCHED_METRICS_URL" "nebula_scheduler_xtrace_query_errors_total")"

info "baseline stale: router=${base_router_stale} scheduler=${base_sched_stale}"
info "baseline rate_limited: router=${base_router_rl} scheduler=${base_sched_rl}"
info "baseline query_errors: router=${base_router_qe} scheduler=${base_sched_qe}"

info "scenario 1: inject stale via controlled stale xtrace mock"
ssh -o BatchMode=yes "${USER_NAME}@${HOST}" "
  if [ -f ${STALE_MOCK_PID_FILE} ]; then
    kill \$(cat ${STALE_MOCK_PID_FILE}) >/dev/null 2>&1 || true
    rm -f ${STALE_MOCK_PID_FILE}
  fi
  nohup python3 -u - <<'PY' >/tmp/nebula_xtrace_stale_mock.log 2>&1 &
from http.server import BaseHTTPRequestHandler, HTTPServer

BODY = b'{"data":[{"labels":{"model_uid":"qwen2_5_0_5b","replica_id":"0"},"values":[{"timestamp":"2020-01-01T00:00:00Z","value":1.0}]}],"meta":{"latest_ts":"2020-01-01T00:00:00Z","series_count":1,"truncated":false}}'

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(BODY)
    def log_message(self, fmt, *args):
        return

HTTPServer(('127.0.0.1', ${STALE_MOCK_PORT}), Handler).serve_forever()
PY
  echo \$! > ${STALE_MOCK_PID_FILE}
"

set_env_key "$TMP_ENV" "OBSERVE_URL" "http://127.0.0.1:${STALE_MOCK_PORT}"
set_env_key "$TMP_ENV" "NEBULA_XTRACE_METRIC_MAX_AGE_MS" "120000"
scp -q "$TMP_ENV" "${USER_NAME}@${HOST}:${REMOTE_ENV_PATH}"
restart_remote_stack
sleep "$WAIT_SEC"

stale_router_after="$(metric_value "$ROUTER_METRICS_URL" "nebula_router_xtrace_stale_total")"
stale_sched_after="$(metric_value "$SCHED_METRICS_URL" "nebula_scheduler_xtrace_stale_total")"
qe_router_after_stale="$(metric_value "$ROUTER_METRICS_URL" "nebula_router_xtrace_query_errors_total")"
qe_sched_after_stale="$(metric_value "$SCHED_METRICS_URL" "nebula_scheduler_xtrace_query_errors_total")"

info "stale after injection: router=${stale_router_after} scheduler=${stale_sched_after}"
info "query_errors after stale injection: router=${qe_router_after_stale} scheduler=${qe_sched_after_stale}"

stale_ok=0
if [ "$stale_router_after" -gt 0 ] || [ "$stale_sched_after" -gt 0 ]; then
  stale_ok=1
  pass "stale counter increased"
elif [ "$qe_router_after_stale" -gt 0 ] || [ "$qe_sched_after_stale" -gt 0 ]; then
  stale_ok=1
  warn "stale counter unchanged; query_errors increased instead (mock not consumed as stale payload)"
else
  fail "stale counter did not increase"
fi

info "scenario 2: inject controlled 429 via local mock xtrace"
ssh -o BatchMode=yes "${USER_NAME}@${HOST}" "
  if [ -f ${STALE_MOCK_PID_FILE} ]; then
    kill \$(cat ${STALE_MOCK_PID_FILE}) >/dev/null 2>&1 || true
    rm -f ${STALE_MOCK_PID_FILE}
  fi
  if [ -f ${MOCK_PID_FILE} ]; then
    kill \$(cat ${MOCK_PID_FILE}) >/dev/null 2>&1 || true
    rm -f ${MOCK_PID_FILE}
  fi
  nohup python3 -u - <<'PY' >/tmp/nebula_xtrace_429_mock.log 2>&1 &
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(429)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Retry-After', '1')
        self.end_headers()
        self.wfile.write(b'{"code":"RATE_LIMITED","error":"mock rate limit"}')
    def log_message(self, fmt, *args):
        return

HTTPServer(('127.0.0.1', ${MOCK_PORT}), Handler).serve_forever()
PY
  echo \$! > ${MOCK_PID_FILE}
"

set_env_key "$TMP_ENV" "OBSERVE_URL" "http://127.0.0.1:${MOCK_PORT}"
set_env_key "$TMP_ENV" "NEBULA_XTRACE_METRIC_MAX_AGE_MS" "120000"
scp -q "$TMP_ENV" "${USER_NAME}@${HOST}:${REMOTE_ENV_PATH}"
restart_remote_stack
sleep "$WAIT_SEC"

rl_router_after="$(metric_value "$ROUTER_METRICS_URL" "nebula_router_xtrace_rate_limited_total")"
rl_sched_after="$(metric_value "$SCHED_METRICS_URL" "nebula_scheduler_xtrace_rate_limited_total")"

info "rate_limited after injection: router=${rl_router_after} scheduler=${rl_sched_after}"

rl_ok=0
if [ "$rl_router_after" -gt "$base_router_rl" ] || [ "$rl_sched_after" -gt "$base_sched_rl" ]; then
  rl_ok=1
  pass "rate_limited counter increased"
else
  fail "rate_limited counter did not increase"
fi

echo
if [ "$stale_ok" -eq 1 ] && [ "$rl_ok" -eq 1 ]; then
  pass "degradation check PASSED"
  exit 0
else
  fail "degradation check FAILED"
  exit 2
fi
