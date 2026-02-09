# Nebula 架构设计

## 1. 背景与动机

### 1.1 Xinference 的核心问题

Xinference 在工程上有大量可复用资产（模型注册表、Hub 下载、模板/解析逻辑等），但其控制面/运行时框架逐渐成为主要瓶颈：

- **调度与路由开销高**：多级 RPC（Supervisor → Worker → ModelActor）导致每次请求/调度都有显著额外延迟。
- **状态脆弱**：关键状态停留在内存结构里，重启恢复成本高，故障域大。
- **进程管理冲突**：Actor 框架的子进程模型与推理引擎（如 vLLM）的内部并发/进程模型容易互相干扰。
- **引擎耦合严重**：把引擎当"库"嵌入并做 monkey-patch，升级引擎版本就会牵动大量兼容代码。
- **异构设备扩展成本高**：设备检测与能力判断散落在多处 if-else。

结论：真正的资产是"模型/协议/业务逻辑"，最大负债是"控制面与引擎强耦合的运行时框架"。Nebula 的方向是重组资产、替换负债。

### 1.2 为什么是 Rust-Native + Python 引擎层

- **Rust 做控制面**（Gateway/Router/Scheduler/Node/MetaStore 适配）：并发、可靠性、可观测性、长时间运行稳定性更可控。
- **Python 做执行面**（Engine Shim + 引擎）：兼容 vLLM/SGLang/Transformers 生态与其快速演进，让"引擎适配"保持薄且可替换。

### 1.3 从 Dynamo 吸收的设计思路

- **控制面 / 执行面分离**：控制面做决策与编排，执行面做推理与资源占用。
- **声明式 + Reconcile**：写入期望状态，让节点侧执行者持续对齐实际状态。
- **标准化契约**：用清晰协议与能力协商（capabilities）替代隐式约定与动态分发。
- **可观测性优先**：路由、调度、引擎、取消、缓存命中等都必须可量化。

---

## 2. 总体架构

### 2.1 组件清单

| 组件 | 语言 | 职责 |
|------|------|------|
| **Gateway** | Rust | OpenAI 兼容 HTTP + SSE；鉴权、参数规范化、错误码映射、usage 汇总 |
| **Router** | Rust | 基于 endpoint 状态做请求路由（least-connections + 健康过滤 + session affinity） |
| **Scheduler** | Rust | 模型放置与副本规划（PlacementPlan），声明式写入 etcd |
| **MetaStore** | etcd | 权威元数据存储（watch/lease/CAS） |
| **Node Daemon** | Rust | watch placement → reconcile 本机引擎进程；上报节点资源；健康检查与自愈 |
| **Engine Shim** | Python | 统一 gRPC EngineService；调用 vLLM/Transformers 原生 API |

### 2.2 关键设计原则

- **Watch-driven reconcile**：Scheduler 只写期望状态，Node 通过 watch 自行执行。
- **三层可靠交付**：notify 快路径（可选） + watch 主路径 + periodic full reconcile 兜底。
- **引擎零侵入**：不再 patch vLLM/SGLang 内部，不与引擎进程模型冲突。
- **capability 驱动兼容**：按能力协商决定可用特性与降级策略。
- **ExecutionContext 贯穿**：Gateway 抽取请求上下文（session/deadline/预算/优先级等）并贯穿 Router 与 Engine 侧。

### 2.3 数据流

```
Client
  │
  ▼
Gateway (8081)  ──  /v1/chat/completions, /v1/responses
  │
  ▼
Router (18081)  ──  endpoint 选择 + 请求代理
  │
  ▼
vLLM (10814)    ──  模型推理
  ▲
  │
Node Daemon     ──  watch placement → 启停 vLLM → 注册 endpoint
  ▲
  │
Scheduler       ──  写入 PlacementPlan 到 etcd
  ▲
  │
etcd (2379)     ──  权威状态存储
```

---

## 3. 元数据（etcd Keyspace）

| Key | 值类型 | 说明 |
|-----|--------|------|
| `/nodes/{node_id}/status` | `NodeStatus` | 节点心跳（lease/TTL） |
| `/models/{model_uid}/spec` | `ModelSpec` | 模型规格（静态或版本化） |
| `/placements/{model_uid}` | `PlacementPlan` | 期望状态，含 `version` 单调递增 |
| `/endpoints/{model_uid}/{replica_id}` | `EndpointInfo` | Node 注册；必须带 `plan_version` |

一致性约束：

- Scheduler 更新 placement 必须 CAS（`expected_version` 与当前一致才允许写入）。
- Router 只使用 `plan_version` 最新的 endpoints，防止旧副本"复活覆盖"。
- Watch 断线后必须重连，重连后必须做一次全量校正（`list_prefix`）。

---

## 4. 调度与放置（Scheduler）

### 4.1 PlacementPlan

- `version: u64`（单调递增）
- `assignments[]`: `replica_id / node_id / gpu_indices / engine_config / role`
- `role`：首期仅启用 `Unified`，schema 预留 `Prefill/Decode`

### 4.2 策略

- **MVP：IdleFirst** — 选择心跳健康的 node，过滤显存不足 GPU，选择综合负载最小的 GPU slot。
- 后续：MemoryAware / Disaggregated / SLA Planner。

---

## 5. 节点侧（Node Daemon）

### 5.1 主循环

- `watch_placements_loop()`：监听 `/placements/`，对 plan 做 reconcile。
- `heartbeat_loop()`：每 3s 上报 `/nodes/{id}/status`（lease 10s）。
- `health_check_loop()`：周期性健康检查引擎进程。
- `periodic_full_reconcile()`：watch 断连/漏事件兜底。

### 5.2 Reconcile 语义

- **期望有、实际无**：启动引擎，等待 ready → 注册 endpoint。
- **期望无、实际有**：优雅关闭（SIGTERM → SIGKILL）。
- **配置变更**：按 `plan_version` 触发滚动更新（首期 stop-then-start）。

---

## 6. 引擎接入

### 6.1 两种模式

- **Unified Gateway（默认）**：请求进入 Gateway → Router → EngineShim（gRPC）。Gateway 保证 `/v1/responses` 的 streaming 事件 1:1 对齐 OpenAI。
- **Engine-Passthrough（可选）**：Gateway 作为反向代理，直接转发到引擎原生 HTTP 服务（如 `vllm serve`）。

### 6.2 EngineService gRPC（概要）

- 生命周期：`HealthCheck / Shutdown / GetModelInfo`
- 推理：`Chat / ChatStream`
- Embedding：`CreateEmbedding`
- 请求管理：`CancelRequest / GetRunningRequests`
- 可观测性：`GetMetrics / GetKVCacheStatus`
- 能力协商：`GetCapabilities`

---

## 7. 对外 API（OpenAI-compatible）

### 7.1 支持的接口

| 接口 | 状态 |
|------|------|
| `POST /v1/chat/completions` (stream/non-stream) | ✅ 已实现 |
| `POST /v1/responses` (stream/non-stream) | ✅ 已实现 |
| `POST /v1/embeddings` | 🚧 返回 501 |

### 7.2 Responses API（重点）

`/v1/responses` 的 streaming 严格对齐 OpenAI：

- SSE 编码，`Content-Type: text/event-stream`
- 事件通过 JSON 内 `type` 字段识别（不依赖 SSE `event:` 行）
- 最小事件序列：`response.created` → `response.output_text.delta`（多次） → `response.completed`
- 每个事件必须包含 `type` 和 `sequence_number`（单调递增）

### 7.3 Tool Calling（best-effort）

- Gateway 默认开启 `tool_call_mode=best_effort`
- 注入工具 schema 到 instructions → 引擎输出 → 解析为 tool call → schema 校验 → 失败则 retry
- 退化策略：重试仍失败时退化为普通文本输出
- 对外事件与对象结构仍然必须是 OpenAI 1:1

---

## 8. Router 信号契约

### 8.1 EndpointInfo（必须项）

```json
{
  "model_uid": "m_xxx",
  "replica_id": 0,
  "plan_version": 12,
  "node_id": "node_1",
  "endpoint_kind": "native_http",
  "status": "ready",
  "last_heartbeat_ms": 1730000000000,
  "base_url": "http://127.0.0.1:10814"
}
```

Router 读取规则：
- 版本过滤：丢弃 `plan_version` 小于当前 placement 的 endpoint
- 健康过滤：`status != ready` 或心跳超时必须下线

### 8.2 EndpointStats（可选）

- `pending_requests`：least-connections 基线
- `prefix_cache_hit_rate`：cache bonus
- `kv_cache_*`：KV-aware 占位信号（首期保留）

---

## 9. 可观测性与错误语义

- **tracing**：`request_id` 全链路；span 维度包含 `model_uid/replica/node_id/engine_name`
- **metrics**：Gateway/Router/Node/EngineShim 各自暴露 Prometheus 指标
- **错误码**：引擎 gRPC error → Gateway 映射为 OpenAI 风格 error（400/429/500）

---

## 10. 里程碑

| 阶段 | 内容 |
|------|------|
| **M0：单机打通** | etcd + gateway/router/node + vLLM；chat/responses（含 streaming + best-effort tool calling） |
| **M1：多机与调度** | scheduler placement + 多 node + endpoint watch 路由 + 自愈 |
| **M2：兼容性增强** | capabilities 完整化、fallback 策略配置化、structured output validate+retry |
| **M3：Agent 友好** | session affinity + prefix cache 指标化 + KV-aware routing |
