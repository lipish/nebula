#!/bin/bash
set -e

# Configuration
ETCD_ENDPOINT="http://127.0.0.1:2379"
GATEWAY_PORT=8081
ROUTER_PORT=18081
NODE_PORT=10814
MODEL_UID="qwen2_5_0_5b"
MODEL_NAME="Qwen/Qwen2.5-0.5B-Instruct"
NODE_ID="node_gpu0"

# Paths (adjust these if running on a different machine)
NEBULA_ROOT=$(pwd)
LOG_DIR="$NEBULA_ROOT/logs"

mkdir -p "$LOG_DIR"

echo "Starting Nebula Service Stack..."
echo "Logs will be written to $LOG_DIR"

# 1. Start Etcd
if pgrep -x "etcd" > /dev/null; then
    echo "Etcd is already running."
else
    echo "Starting Etcd..."
    nohup ~/bin/etcd --advertise-client-urls http://0.0.0.0:2379 --listen-client-urls http://0.0.0.0:2379 > "$LOG_DIR/etcd.log" 2>&1 &
    sleep 2
fi

# 2. Start Router
echo "Starting Nebula Router..."
nohup ./target/release/nebula-router \
    --listen-addr "0.0.0.0:$ROUTER_PORT" \
    --etcd-endpoint "$ETCD_ENDPOINT" > "$LOG_DIR/router.log" 2>&1 &

# 2.5 Start Scheduler
echo "Starting Nebula Scheduler..."
nohup ./target/release/nebula-scheduler \
    --etcd-endpoint "$ETCD_ENDPOINT" \
    --default-node-id "$NODE_ID" > "$LOG_DIR/scheduler.log" 2>&1 &

# 3. Start Gateway
echo "Starting Nebula Gateway..."
export RUST_LOG=info
export NEBULA_GATEWAY_ADDR="0.0.0.0:$GATEWAY_PORT"
export NEBULA_ROUTER_URL="http://127.0.0.1:$ROUTER_PORT"
nohup ./target/release/nebula-gateway > "$LOG_DIR/gateway.log" 2>&1 &

# 4. Start Node (with ModelScope if needed)
echo "Starting Nebula Node ($NODE_ID)..."
# Check if VLLM_USE_MODELSCOPE is needed (simple check for now)
export VLLM_USE_MODELSCOPE=True

nohup ./target/release/nebula-node \
    --node-id "$NODE_ID" \
    --etcd-endpoint "$ETCD_ENDPOINT" \
    --vllm-bin "$HOME/.local/bin/vllm" \
    --vllm-config "$NEBULA_ROOT/qwen.yaml" \
    --vllm-cwd "$NEBULA_ROOT" \
    --vllm-port "$NODE_PORT" \
    --ready-timeout-secs 1200 > "$LOG_DIR/node_$NODE_ID.log" 2>&1 &

echo "All services started!"
echo "Gateway: http://127.0.0.1:$GATEWAY_PORT"
echo "Router:  http://127.0.0.1:$ROUTER_PORT"
echo "Node:    $NODE_ID (Model: $MODEL_NAME)"
