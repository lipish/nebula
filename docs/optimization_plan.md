# Nebula 优化计划（借鉴 AIBrix）

> 基于对 AIBrix 项目的深度分析，结合 Nebula 当前代码状态，制定的分层优化路线。

## 核心判断

Nebula 和 AIBrix 解决同一层问题（LLM serving 控制面），但运行假设完全不同：

| | AIBrix | Nebula |
|--|--------|--------|
| **底座** | K8s（Pod/Deployment/HPA/Envoy 全套） | etcd + 自研 Daemon（裸机/docker-compose） |
| **扩缩容** | K8s HPA/KPA 做 Pod 级别 | Scheduler 写 PlacementPlan，Node reconcile |
| **网关** | Envoy Gateway + External Processing | 自研 Rust Gateway + Router |
| **引擎管理** | Sidecar（AI Runtime） | Node Daemon 直接管理子进程/容器 |

Nebula 的优化不应复刻 AIBrix 的 feature list，而是**利用自身架构优势，把 AIBrix 验证过的高价值思路用 Nebula 的方式实现**。

---

## 第一层：信号基础设施（一切优化的前提）

当前最大短板是**缺数据**。Router 路由决策只有 `pending_requests` 一个信号，Scheduler 放置决策只有 GPU 显存一个信号。

### 1.1 引擎指标采集（Engine Stats Pipeline）

**现状**：`EndpointStats` 已定义 `prefix_cache_hit_rate`、`kv_cache_used_bytes` 等字段，但没有任何地方填充。

**设计**：Node Daemon heartbeat 循环中，对每个 running engine 主动拉取 vLLM `/metrics`，解析关键指标，写入 etcd。

```
heartbeat_loop (每 3s)
  ├── read_gpu_statuses()          ← 已有
  ├── scrape_engine_metrics()      ← 新增
  │     对每个 running engine:
  │       GET {base_url}/metrics
  │       解析: pending, running, kv_cache_usage, prefix_cache_hit_rate
  │       写入 EndpointStats → etcd /stats/{model_uid}/{replica_id}
  └── register_endpoint()          ← 已有
```

**改动范围**：
- `nebula-node/src/heartbeat.rs` — 增加 `scrape_engine_metrics()`
- `nebula-router/src/sync.rs` — 增加 watch `/stats/` 前缀，同步到 `Router.stats`

**优先级**：最高。这是所有后续优化的数据管道，没有它 cache-aware 路由、KV-aware 路由、autoscaling 全都无法实现。

### 1.2 Router 侧请求级指标

**现状**：Gateway `track_requests` 只统计 total/inflight/status code，无延迟、无 per-model 维度。

**设计**：在 Router 的 `proxy_chat_completions` handler 中记录：
- **E2E latency**（请求进入到响应完成）
- **TTFT**（流式场景，第一个 SSE chunk 到达的时间）
- 按 `model_uid` 维度聚合

实现方式：`DashMap<String, AtomicHistogram>`，histogram 用固定 bucket（1ms, 5ms, 10ms, 50ms, 100ms, 500ms, 1s, 5s, 10s, 30s），Prometheus 标准做法。

**改动范围**：
- `nebula-router/src/handlers.rs` — proxy 前后记录时间戳
- `nebula-router/src/metrics.rs` — histogram 结构和 Prometheus 格式输出

---

## 第二层：路由智能化（投入产出比最高）

AIBrix 论文数据：prefix-cache-aware 路由 P99 降低 79%。

### 2.1 路由策略插件化

**现状**：`Router::route()` 硬编码 least-pending 逻辑。

**设计**：

```rust
pub trait RoutingStrategy: Send + Sync {
    fn select(&self, candidates: &[Candidate]) -> Option<usize>;
    fn name(&self) -> &'static str;
}

pub struct Candidate {
    pub endpoint: EndpointInfo,
    pub stats: Option<EndpointStats>,
}
```

Router 负责过滤（model_uid 匹配、Ready 状态、plan_version 一致），Strategy 只负责从候选列表中选一个。

**初始策略集**：
- `LeastPending` — 现有逻辑直接迁移
- `LeastKvCache` — 选 `kv_cache_used_bytes` 最低的（依赖 1.1）
- `PrefixCacheAware` — 选 `prefix_cache_hit_rate` 最高的，低于阈值时 fallback 到 LeastPending（依赖 1.1）

**改动范围**：
- `nebula-router/src/strategy.rs` — 新增 trait + 3 个实现
- `nebula-router/src/lib.rs` — Router 持有 `Arc<dyn RoutingStrategy>`，route() 重构
- `nebula-router/src/args.rs` — 增加 `--routing-strategy` 参数

### 2.2 不实现 Random/RoundRobin

LLM 请求计算量差异巨大（10 token vs 10000 token），盲目均分只会导致负载不均。AIBrix 虽实现了 random 但论文从未推荐使用。精力应放在 LeastKvCache 和 PrefixCacheAware 上。

---

## 第三层：Scheduler 从静态放置到动态调节

### 3.1 现状

当前 Scheduler 是纯事件驱动的一次性放置器：watch `/model_requests/` → 选 node → 写 PlacementPlan → 完事。之后不再管理。引擎挂了没人知道，负载变化不会调整副本数。

### 3.2 引入 Reconcile Loop

```
scheduler_reconcile_loop (每 30s)
  ├── 读取所有 /placements/
  ├── 读取所有 /endpoints/ 和 /stats/
  ├── 对每个 model:
  │     ├── 检查健康：endpoint 心跳超时 → 标记异常
  │     ├── 检查负载：avg(kv_cache_usage) > 80% → 需要扩容
  │     ├── 检查空闲：avg(pending_requests) == 0 持续 5min → 可以缩容
  │     └── 生成调整决策 → 更新 PlacementPlan
  └── 写入 etcd（CAS 保证一致性）
```

**第一阶段：健康自愈**
- endpoint 心跳超时 > 30s → 从 PlacementPlan 移除，触发 Node 侧清理
- `replicas` 要求未满足 → 重新选 node 补充

**第二阶段：负载驱动扩缩容**
- 基于 `kv_cache_usage` 和 `pending_requests` 滑动窗口均值自动调整 `replicas`
- `ModelLoadRequest` 增加 `min_replicas` / `max_replicas` 字段

**改动范围**：
- `nebula-scheduler/src/main.rs` — 增加 reconcile loop（与现有 watch loop 并行）
- `nebula-scheduler/src/planner.rs` — 增加 `reconcile_placements()`
- `nebula-common/src/model_request.rs` — 增加 `min_replicas` / `max_replicas`

---

## 第四层：Node 侧健壮性

### 4.1 引擎健康检查

**现状**：Node 启动引擎后只做一次 `wait_engine_ready`，之后不再检查。引擎 OOM 或 hang 住不会被发现。

**设计**：heartbeat loop 中对每个 running engine 探测 `/health`，连续 3 次失败 → 标记 Unhealthy → 尝试重启。

**改动范围**：
- `nebula-node/src/reconcile.rs` — `RunningModel` 增加 `consecutive_failures` 和 `base_url`
- `nebula-node/src/heartbeat.rs` — 增加健康检查逻辑

### 4.2 GPU 状态增强

**现状**：`nvidia-smi` 只查 `memory.total,memory.used`。

**设计**：增加 `temperature.gpu` 和 `utilization.gpu`，只上报数据，评估逻辑留给 Scheduler 策略层。

**改动范围**：
- `nebula-common/src/node_status.rs` — `GpuStatus` 增加 `temperature` 和 `utilization_gpu`
- `nebula-node/src/gpu.rs` — nvidia-smi 查询增加字段

---

## 第五层：Gateway 防护

### 5.1 Admission Control（基于负载的背压）

不做传统 Token Bucket 限流（LLM 请求成本差异巨大，QPS 限流意义不大）。

Router 路由时，如果所有 endpoint 的 `kv_cache_usage > 95%`，直接拒绝新请求（返回 429 + Retry-After）。这是基于真实负载的背压。

**改动范围**：
- `nebula-router/src/lib.rs` — route() 返回 `Result<EndpointInfo, RouteError>`
- `nebula-router/src/handlers.rs` — 根据 RouteError 类型返回 503 或 429

### 5.2 简单重试

当前 route 失败直接返回 503。可加简单 retry：route 失败后 sleep 100ms 重试一次，无需复杂排队系统。

---

## 第六层：可观测性基础设施

### 6.1 统一 Metrics 暴露（Prometheus 格式）

**现状**：Gateway 有基础计数器但未暴露 `/metrics`；Engine Stats Pipeline 写 etcd 供 Router 实时路由，但无历史回溯能力；容器/镜像资产无记录。

**设计原则**：
- etcd 只存控制面数据（placement、endpoint、stats），不存可观测性数据
- 每个组件暴露 `/metrics` endpoint，由 Prometheus 统一采集和持久化

**各组件指标**：

```
Node /metrics:
  nebula_container_status{model_uid, replica_id, image}
  nebula_container_restart_total{model_uid}
  nebula_gpu_temperature{gpu_index}
  nebula_gpu_utilization{gpu_index}
  nebula_engine_kv_cache_usage{model_uid}
  nebula_engine_pending_requests{model_uid}

Router /metrics:
  nebula_route_total{model_uid, status}
  nebula_route_latency_seconds{model_uid, quantile}

Gateway /metrics:
  nebula_request_total{status_code}
  nebula_request_concurrent
```

**改动范围**：
- 各 crate 增加 `metrics.rs`，使用 `prometheus` crate 注册指标
- 各 HTTP server 增加 `GET /metrics` handler

### 6.2 日志集中收集

**设计**：各组件 `tracing` 接入 OpenTelemetry exporter → 推送到 Loki。容器日志通过 Docker logging driver 或 Loki Docker plugin 采集。

### 6.3 分布式 Tracing

**设计**：请求从 Gateway → Router → Engine 的完整链路，通过 OpenTelemetry SDK 注入 trace context，推送到 Jaeger/Tempo。

---

## 实施顺序与依赖关系

```
第一层：信号基础设施
  1.1 Engine Stats Pipeline ──────┐
  1.2 Router 请求级指标           │
                                  ▼
第二层：路由智能化                │
  2.1 策略插件化 ◄────────────────┘
      ├── LeastPending (迁移现有)
      ├── LeastKvCache (依赖 1.1)
      └── PrefixCacheAware (依赖 1.1)

第三层：Scheduler 动态调节
  3.1 健康自愈 (依赖 1.1)
  3.2 负载驱动扩缩容 (依赖 1.1 + 3.1)

第四层：Node 健壮性
  4.1 引擎健康检查
  4.2 GPU 状态增强

第五层：Gateway 防护
  5.1 Admission Control (依赖 1.1)
  5.2 简单重试
```

**关键路径**：1.1 → 2.1 → 3.1。这三步做完，Nebula 从"能用"变成"好用"。

## 工作量估算

| 阶段 | 工作量 | 说明 |
|------|--------|------|
| 1.1 Engine Stats Pipeline | 2 天 | scrape + 写 etcd + Router 侧 sync |
| 1.2 请求级指标 | 1 天 | histogram + TTFT 记录 |
| 2.1 路由策略插件化 | 2 天 | trait + 3 策略 + 参数 + 测试 |
| 3.1 Scheduler 健康自愈 | 2 天 | reconcile loop + 补充副本 |
| 3.2 负载驱动扩缩容 | 3 天 | 扩缩容决策 + min/max replicas |
| 4.1 引擎健康检查 | 1 天 | /health 探测 + 重启逻辑 |
| 4.2 GPU 状态增强 | 0.5 天 | nvidia-smi 字段扩展 |
| 5.1 Admission Control | 1 天 | 背压逻辑 + 429 返回 |
| **合计** | **~12.5 天** | |

前 5 天（1.1 + 1.2 + 2.1）完成后即可看到明显效果。
