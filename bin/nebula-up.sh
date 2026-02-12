#!/bin/bash
set -e

# Configuration
ETCD_ENDPOINT="http://127.0.0.1:2379"
GATEWAY_PORT=8081
ROUTER_PORT=18081
NODE_PORT=10824
MODEL_UID="qwen2_5_0_5b"
MODEL_NAME="Qwen/Qwen2.5-0.5B-Instruct"
NODE_ID="node_gpu0"

# Paths (adjust these if running on a different machine)
NEBULA_ROOT=$(cd "$(dirname "$0")/.." && pwd)
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
nohup "$NEBULA_ROOT/target/release/nebula-router" \
    --listen-addr "0.0.0.0:$ROUTER_PORT" \
    --etcd-endpoint "$ETCD_ENDPOINT" > "$LOG_DIR/router.log" 2>&1 &

# 2.5 Start Scheduler
echo "Starting Nebula Scheduler..."
nohup "$NEBULA_ROOT/target/release/nebula-scheduler" \
    --etcd-endpoint "$ETCD_ENDPOINT" \
    --default-node-id "$NODE_ID" \
    --default-port "$NODE_PORT" > "$LOG_DIR/scheduler.log" 2>&1 &

# 3. Start Gateway
nohup "$NEBULA_ROOT/target/release/nebula-gateway" \
    --listen-addr "0.0.0.0:$GATEWAY_PORT" \
    --router-url "http://127.0.0.1:$ROUTER_PORT" > "$LOG_DIR/gateway.log" 2>&1 &

# 4. Start Node (docker mode — all vLLM runs inside containers)
VLLM_IMAGE="vllm/vllm-openai:v0.11.0"
VLLM_MODEL_DIR="/DATA/Model"

echo "Starting Nebula Node ($NODE_ID) [docker: $VLLM_IMAGE]..."
nohup "$NEBULA_ROOT/target/release/nebula-node" \
    --node-id "$NODE_ID" \
    --etcd-endpoint "$ETCD_ENDPOINT" \
    --vllm-docker-image "$VLLM_IMAGE" \
    --vllm-model-dir "$VLLM_MODEL_DIR" \
    --vllm-port "$NODE_PORT" \
    --vllm-use-modelscope \
    --ready-timeout-secs 1200 > "$LOG_DIR/node_$NODE_ID.log" 2>&1 &

echo "All services started!"
echo "Gateway: http://127.0.0.1:$GATEWAY_PORT"
echo "Router:  http://127.0.0.1:$ROUTER_PORT"
echo "Node:    $NODE_ID (Model: $MODEL_NAME)"
