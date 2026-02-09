# 2x RTX 5090 Deployment Guide

Current Status: The `nebula-node` binary manages a single vLLM instance. To utilize 2x GPUs for different models (or replicas), we treat them as two logical nodes.

## 1. Topologgy

-   **Physical Node**: 1x Machine (Dual 5090)
-   **Logical Nodes**:
    -   `node_gpu0`: Bound to GPU 0 (e.g., Qwen2.5-7B)
    -   `node_gpu1`: Bound to GPU 1 (e.g., Llama-3-8B)
-   **Ports**:
    -   Gateway: 8081
    -   Router: 18081
    -   vLLM (GPU 0): 10814
    -   vLLM (GPU 1): 10815

## 2. Step-by-Step Deployment

### Step 1: Start Etcd

```bash
docker run -d --rm --name etcd \
  -p 2379:2379 \
  quay.io/coreos/etcd:v3.5.0 \
  /usr/local/bin/etcd \
  --advertise-client-urls http://0.0.0.0:2379 \
  --listen-client-urls http://0.0.0.0:2379
```

### Step 2: Start Control Plane

```bash
# Gateway
./target/debug/nebula-gateway --listen 0.0.0.0:8081 &

# Router
./target/debug/nebula-router --listen 0.0.0.0:18081 &
```

### Step 3: Start Logical Nodes (The "Missing" Link)

We manually isolate GPUs using `CUDA_VISIBLE_DEVICES`.

**Logical Node 0 (GPU 0):**

```bash
CUDA_VISIBLE_DEVICES=0 ./target/debug/nebula-node \
  --node-id node_gpu0 \
  --etcd-endpoint http://127.0.0.1:2379 \
  --vllm-port 10814 &
```

**Logical Node 1 (GPU 1):**

```bash
CUDA_VISIBLE_DEVICES=1 ./target/debug/nebula-node \
  --node-id node_gpu1 \
  --etcd-endpoint http://127.0.0.1:2379 \
  --vllm-port 10815 &
```

### Step 4: Schedule Models

We need to tell the scheduler to place specific models on these specific logical nodes.

**Model A on GPU 0:**

```bash
./target/debug/nebula-scheduler \
  --model-uid qwen2_5_7b \
  --node-id node_gpu0 \
  --port 10814 \
  --engine-config-path /path/to/qwen.yaml
```

**Model B on GPU 1:**

```bash
./target/debug/nebula-scheduler \
  --model-uid llama_3_8b \
  --node-id node_gpu1 \
  --port 10815 \
  --engine-config-path /path/to/llama.yaml
```

## 3. What is Missing?

To make this "Production Ready", we are missing:

1.  **Unified Node Manager**: Instead of running two `nebula-node` processes manually, a single Daemon should detect 2 GPUs and spawn 2 child workers automatically.
2.  **Resource Discovery**: The Scheduler currently blindly accepts `--node-id`. It should query etcd to see "node_gpu0 has 24GB VRAM" and refuse to place a 70B model there.
3.  **Port Management**: We manually assigned 10814/10815. The system should auto-allocate ports.

## 4. Verification

After running the above:

```bash
curl http://127.0.0.1:8081/v1/chat/completions \
  -d '{"model": "qwen2_5_7b", "messages": [{"role":"user","content":"hi"}]}'
```

The router will see `qwen2_5_7b` is on `node_gpu0` (via etcd) and route to `127.0.0.1:10814`.
