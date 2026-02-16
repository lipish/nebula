# Nebula Gateway 优化方案（内部调度语义）

> 范围约束：本文仅面向 Nebula 现有 `gateway + router + etcd` 内部调度链路，不引入 provider-domain 语义，不替换现有架构。

## 1. 当前主要问题

### 1.1 可用性问题

1. **无上游快速重试**：上游瞬时失败会直接对外暴露 502/503。
2. **无 endpoint 短时熔断**：连续失败副本可能被反复命中。
3. **超时策略过粗**：连接超时与总超时未分层，缺首包超时治理。

### 1.2 稳定性问题

1. **背压策略偏硬**：仅在“全部 endpoint 超载”时拒绝，缺渐进降载。
2. **缺失 stats 时路由偏乐观**：无 fresh stats 的副本可能被当作“低负载”。
3. **请求体无上限**：极端流量下存在内存压力风险。

### 1.3 可观测性问题

1. 缺 `retry`、`circuit`、`fallback`、`upstream_error_kind` 等关键防护指标。
2. 无法形成“策略调参 → 指标反馈 → 再调参”的闭环。
3. 术语易混淆：xtrace/nebula-observe 是数据查询后端，观测面板应由前端承载。

---

## 2. 优化原则

1. **保持语义一致**：围绕 model/endpoint/replica，不引入 provider 抽象。
2. **最小改动优先**：先补失败处理链路，再做策略升级。
3. **可观测先行**：每个防护动作必须有指标与日志。
4. **默认安全**：防护参数有保守默认值，支持环境变量覆盖。

---

## 3. 分阶段实施计划

## P0：快速止血（建议本周）

### P0-1 一次快速重试（只重试可重试错误）

- 行为：首次失败后，延迟 50~100ms 抖动，最多重试 1 次。
- 触发条件：网络错误、超时、上游 5xx。
- 排除条件：4xx、鉴权错误、请求格式错误。

### P0-2 重试时排除首个失败 endpoint

- 行为：第二次路由必须排除第一次失败副本，避免“打同一坏点”。

### P0-3 请求体大小上限

- 建议默认：`4MB`（可配置）。
- 超限行为：直接 `413 Payload Too Large`，不进入上游调用。

### P0-4 补齐防护核心指标

- Router：
	- `nebula_router_retry_total`
	- `nebula_router_retry_success_total`
	- `nebula_router_upstream_error_total{kind}`
	- `nebula_router_request_too_large_total`
- Gateway：
	- `nebula_gateway_upstream_error_total{kind}`
	- `nebula_gateway_request_too_large_total`

### P0-5 参数化配置（已落地）

- Router：
	- `NEBULA_ROUTER_MAX_REQUEST_BODY_BYTES`（默认 `4194304`）
	- `NEBULA_ROUTER_RETRY_MAX`（默认 `1`）
	- `NEBULA_ROUTER_RETRY_BACKOFF_MS`（默认 `75`）
- Gateway：
	- `NEBULA_GATEWAY_MAX_REQUEST_BODY_BYTES`（默认 `4194304`）

---

## P1：稳定性增强（建议下周）

### P1-1 endpoint 短时熔断

- 建议默认：连续失败 `N=3` 次，熔断 `T=30s`。
- 熔断窗口内：不参与候选。
- 半开策略：窗口结束后放行小流量探测。

### P1-2 背压从“硬拒绝”升级为“候选缩减”

- 先剔除高风险 endpoint（高 KV 使用率、熔断态、stale stats）。
- 仅在候选为空时返回 429/503。

### P1-3 无 fresh stats endpoint 降权

- 将“无数据”视为风险，不再默认优选。

---

## P2：运维可控性与参数化（建议第三周）

### P2-1 超时分层

- `connect_timeout_ms`
- `first_byte_timeout_ms`
- `request_timeout_ms`

### P2-2 防护指标补全

- `nebula_router_circuit_open_total`
- `nebula_router_admission_reject_total{reason}`
- `nebula_router_route_fallback_total{reason}`

### P2-3 文档化默认参数与调参手册

- 给出压测下的推荐参数区间与回滚阈值。

---

## 4. 验收标准（DoD）

## P0 DoD

1. 网络错误/5xx 可触发一次重试，且最多一次。
2. 重试阶段不会命中首次失败 endpoint。
3. 超大请求体返回 413。
4. 指标可在 `/metrics` 中观测到，并随请求递增。

## P1 DoD

1. 连续失败达到阈值后 endpoint 进入熔断态。
2. 熔断窗口内该 endpoint 不参与路由。
3. 发生过载时优先缩减候选，而非立即全局拒绝。
4. 无 fresh stats endpoint 命中率显著下降。

## P2 DoD

1. 三类超时可独立配置并生效。
2. 防护指标可支持策略调参与回归对比。

---

## 5. 测试矩阵（最小集）

1. **单点失败**：单 endpoint 失败后重试仍失败，返回符合预期。
2. **双副本一坏一好**：首次失败后二次命中健康副本并成功。
3. **全部过载**：按策略返回 429（含 Retry-After）。
4. **SSE 慢首包**：首包超时策略生效。
5. **超大请求体**：返回 413，不产生上游流量。
6. **熔断恢复**：熔断窗口结束后可半开探测并恢复。

---

## 6. 任务卡模板（用于排期）

### 模板

- **目标**：
- **改动范围**：
- **配置项**：
- **验收标准**：
- **观测指标**：
- **回滚条件**：

### 示例：P0-1 一次快速重试

- **目标**：降低瞬时抖动导致的 5xx 暴露率。
- **改动范围**：router/gateway 请求转发路径。
- **配置项**：`NEBULA_ROUTER_RETRY_MAX=1`，`NEBULA_ROUTER_RETRY_BACKOFF_MS=75`。
- **验收标准**：可重试错误触发且仅触发一次重试；不可重试错误不重试。
- **观测指标**：`nebula_router_retry_total`、`nebula_router_retry_success_total`、`nebula_router_upstream_error_total{kind}`。
- **回滚条件**：重试引入 P99 明显恶化且成功率无提升。

---

## 7. 架构边界说明（必须保留）

本方案仅借鉴成熟防护机制，不改变 Nebula 现有内部调度边界：

1. 不替换 Gateway 架构。
2. 不引入 provider pool 语义。
3. 不改变 `router + placement + endpoint` 为核心的控制面模型。
