# Nebula 部署指南

## 1. 环境要求

| 依赖 | 版本/说明 |
|------|-----------|
| **Rust** | stable（用于编译 nebula 各组件） |
| **etcd** | v3.x，监听 `127.0.0.1:2379` |
| **vLLM** | 支持 `vllm serve` 的版本 |
| **NVIDIA GPU** | 需要足够显存加载目标模型 |
| **CUDA** | 与 vLLM 兼容的版本 |

## 2. 编译

```bash
cargo build --workspace
```

编译产物位于 `target/debug/`（或 `target/release/`），包含：
- `nebula-scheduler`
- `nebula-node`
- `nebula-router`
- `nebula-gateway`

## 3. 启动 etcd

使用 Docker 快速启动：

```bash
docker run -d --name etcd \
  -p 2379:2379 -p 2380:2380 \
  quay.io/coreos/etcd:v3.5.0 \
  /usr/local/bin/etcd \
  --advertise-client-urls http://0.0.0.0:2379 \
  --listen-client-urls http://0.0.0.0:2379
```

验证健康：

```bash
curl http://127.0.0.1:2379/health
# 预期：{"health":"true","reason":""}
```

## 4. 按顺序启动服务

### 4a. Scheduler — 写入 PlacementPlan

```bash
./target/debug/nebula-scheduler \
  --model-uid qwen2_5_0_5b \
  --model-name Qwen/Qwen2.5-0.5B-Instruct \
  --node-id $(hostname) \
  --engine vllm \
  --port 10814
```

确认输出包含 `placed: key=/placements/qwen2_5_0_5b ...`。

### 4b. Node — 拉起 vLLM 并注册 endpoint

```bash
./target/debug/nebula-node \
  --node-id $(hostname) \
  --etcd-endpoints http://127.0.0.1:2379
```

Node 会：
1. Watch `/placements/` 获取分配给本机的 assignment
2. 启动 vLLM 进程（`vllm serve`）
3. 等待 vLLM ready 后注册 `/endpoints/{model_uid}/{replica_id}`

确认日志包含 `registered endpoint ... base_url=http://127.0.0.1:10814`。

### 4c. 验证 vLLM 就绪

```bash
curl http://127.0.0.1:10814/v1/models
# 预期：200 OK，包含模型名称
```

### 4d. Router — 路由请求到 endpoint

```bash
./target/debug/nebula-router \
  --listen 0.0.0.0:18081 \
  --etcd-endpoints http://127.0.0.1:2379
```

验证健康：

```bash
curl http://127.0.0.1:18081/healthz
# 预期：ok
```

### 4e. Gateway — 对外提供 OpenAI API

```bash
./target/debug/nebula-gateway \
  --listen 0.0.0.0:8081 \
  --router-url http://127.0.0.1:18081
```

验证健康：

```bash
curl http://127.0.0.1:8081/healthz
# 预期：ok
```

## 5. 快速验证

### 非流式 Chat

```bash
curl -s http://127.0.0.1:8081/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello"}]
  }' | python3 -m json.tool
```

### 流式 Chat

```bash
curl -N http://127.0.0.1:8081/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true
  }'
```

### 流式 Responses

```bash
curl -N http://127.0.0.1:8081/v1/responses \
  -H "Content-Type: application/json" \
  -d '{
    "model": "Qwen/Qwen2.5-0.5B-Instruct",
    "input": "Hello",
    "stream": true
  }'
```

## 6. 端口汇总

| 服务 | 默认端口 | 说明 |
|------|----------|------|
| etcd | 2379 | 元数据存储 |
| vLLM | 10814 | 模型推理（由 node 自动启动） |
| Router | 18081 | 请求路由 |
| Gateway | 8081 | 对外 API 入口 |

## 7. 常见问题

### 端口被占用

启动前检查端口是否空闲：

```bash
ss -tlnp | grep -E '(8081|18081|10814)'
```

如有残留进程，先清理：

```bash
pkill -f 'nebula-router'
pkill -f 'nebula-gateway'
pkill -f 'nebula-node'
pkill -f 'vllm serve'
```

### vLLM 孤儿进程

`pkill -f 'vllm serve'` 可能无法杀掉 vLLM 的子进程（如 `VLLM::EngineCore`）。需要：

```bash
# 查找残留的 vLLM 相关进程
nvidia-smi  # 查看 GPU 占用
ps aux | grep -i vllm

# 手动 kill 孤儿进程
kill <pid>
# 如果 TERM 无效
kill -9 <pid>
```

### Router 启动无输出

Router 如果绑定端口失败会静默退出。启动时建议加 `RUST_BACKTRACE=1`：

```bash
RUST_BACKTRACE=1 ./target/debug/nebula-router --listen 0.0.0.0:18081 --etcd-endpoints http://127.0.0.1:2379
```
