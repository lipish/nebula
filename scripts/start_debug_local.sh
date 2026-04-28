#!/bin/bash
pkill -9 -f nebula-node
pkill -9 -f vllm
rm -f ~/nebula/logs/node_debug.log
export VLLM_USE_MODELSCOPE=True
nohup ./target/release/nebula-node \
    --node-id node_gpu0 \
    --etcd-endpoint http://127.0.0.1:2379 \
    --vllm-bin /home/lipeng/.local/bin/vllm \
    --vllm-config /home/lipeng/nebula/qwen.yaml \
    --vllm-cwd /home/lipeng/nebula \
    --vllm-host 0.0.0.0 \
    --vllm-port 10814 \
    > ~/nebula/logs/node_debug.log 2>&1 &
echo "Started node"
