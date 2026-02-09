# Nebula 端到端测试记录

## 测试日期

2025-02-08

## 测试环境

- **模型**：Qwen/Qwen2.5-0.5B-Instruct
- **引擎**：vLLM（native HTTP passthrough 模式）
- **元数据存储**：etcd v3，`127.0.0.1:2379`
- **GPU**：NVIDIA（单机单卡）

## 服务拓扑

```
Gateway (8081) → Router (18081) → vLLM (10814)
                                      ▲
                              Node Daemon 管理
                                      ▲
                              Scheduler 写入 PlacementPlan → etcd (2379)
```

---

## 1. 编译

| 步骤 | 结果 |
|------|------|
| `cargo build --workspace` | ✅ 成功（仅有少量 `dead_code` warning，不影响运行） |

## 2. 基础服务检查

| 检查项 | 结果 |
|--------|------|
| etcd 健康 (`/health`) | ✅ `{"health":"true","reason":""}` |
| 端口 8081/18081/10814 空闲 | ✅ 清理后确认无冲突 |
| GPU 无残留进程 | ✅ 清理后确认（见下方说明） |

### 清理过程

- 初始状态：机器上残留一批 nebula/node/router/gateway 以及一个 vLLM 实例。
- `pkill -f` 清理大部分进程后，仍残留一个孤儿 `VLLM::EngineCore`（PID 3015412，占用约 78GB 显存，PPID=1）。
- 原因：`pkill -f 'vllm serve'` 无法匹配 vLLM 的子进程名。
- 解决：手动 `kill 3015412`（TERM），确认 `nvidia-smi` 不再显示占用。

## 3. 按顺序启动

| 步骤 | 组件 | 结果 |
|------|------|------|
| 4a | Scheduler | ✅ 输出 `placed: key=/placements/qwen2_5_0_5b ...` |
| 4b | Node | ✅ 日志确认 `registered endpoint ... base_url=http://127.0.0.1:10814` |
| 4c | vLLM ready | ✅ `GET /v1/models` 返回 200，包含 `Qwen2.5-0.5B-Instruct` |
| 4d | Router | ✅ `18081` 监听，`/healthz` 返回 `ok` |
| 4e | Gateway | ✅ `8081` 监听，`/healthz` 返回 `ok` |

### Router 启动问题

- 首次启动时遇到 `Address already in use (os error 98)`（`18081` 被残留进程占用）。
- 清理占用后再次启动，正常监听。
- **注意**：Router 绑定端口失败时几乎无输出，建议启动时加 `RUST_BACKTRACE=1`。

## 4. 端到端请求验证

### 4a. Gateway `/v1/chat/completions`（非流式）

- **结果**：✅ 返回 JSON 正常
- **响应内容**：`choices[0].message.content` = `"Hello! How can I assist you today?"`

### 4b. Gateway `/v1/chat/completions`（流式）

- **结果**：✅ SSE 流正常
- **格式**：符合 `data: {"choices":[{"delta":{"content":"..."}}]}` 形式
- **示例片段**：

```text
data: {"id":"chatcmpl-...","object":"chat.completion.chunk",...,"choices":[{"delta":{"content":"Hello"}}]}
data: {"id":"chatcmpl-...","object":"chat.completion.chunk",...,"choices":[{"delta":{"content":"!"}}]}
...
data: [DONE]
```

### 4c. Gateway `/v1/responses`（流式）

- **结果**：✅ SSE 流正常
- **事件序列**：`response.created` → `response.output_text.delta`（多次） → `response.completed`
- **示例片段**：

```text
data: {"type":"response.created", ...}
data: {"type":"response.output_text.delta", "delta":"Hello", ...}
data: {"type":"response.output_text.delta", "delta":"!", ...}
...
data: {"type":"response.completed", ...}
```

### 4d. 直接请求 Router `/v1/chat/completions`

- **结果**：✅ 返回 JSON 正常
- **对比**：与通过 Gateway 的结果一致（`"Hello! How can I assist you today?"`）
- **结论**：Gateway 确实经 Router 转发，链路完整

## 5. 测试结论

| 验证项 | 状态 |
|--------|------|
| 编译 | ✅ |
| etcd 健康 | ✅ |
| vLLM 就绪 | ✅ |
| Router `/healthz` | ✅ |
| Gateway `/healthz` | ✅ |
| Gateway 非流式 chat | ✅ |
| Gateway 流式 chat (SSE) | ✅ |
| Gateway `/v1/responses` 流式 | ✅ |
| 直接请求 Router 对比 | ✅ |

**整条链路 `Gateway(8081) → Router(18081) → vLLM(10814)` 已完全跑通，非流式和流式均正常。**

## 6. 已知问题与注意事项

1. **vLLM 孤儿进程**：`pkill -f 'vllm serve'` 无法匹配 vLLM 子进程（如 `VLLM::EngineCore`），需手动 `kill` PID。
2. **Router 端口冲突静默退出**：绑定失败时几乎无输出，需加 `RUST_BACKTRACE=1` 排查。
3. **8080 端口**：测试机器上被其他服务占用，不影响本次使用 8081 的 Gateway。
4. **Embeddings**：`/v1/embeddings` 当前返回 501 Not Implemented，待后续接入 embedding 模型。
