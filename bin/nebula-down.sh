#!/bin/bash

echo "Stopping Nebula Service Stack..."

echo "Stopping Gateway..."
pkill -f "nebula-gateway" || echo "Gateway not running"

echo "Stopping BFF..."
pkill -f "nebula-bff" || echo "BFF not running"

echo "Stopping Scheduler..."
pkill -f "nebula-scheduler" || echo "Scheduler not running"

echo "Stopping Router..."
pkill -f "nebula-router" || echo "Router not running"

echo "Stopping Node Daemon..."
pkill -f "nebula-node" || echo "Node Daemon not running"

echo "Stopping vLLM Engine..."
pkill -f "vllm" || echo "vLLM not running"

# Optional: Stop Etcd? Usually we might want to keep it running, but for dev simplicity:
# echo "Stopping Etcd..."
# pkill -x "etcd" || echo "Etcd not running"

echo "Cleanup complete."
