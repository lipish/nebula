# Gateway 可观测性面板规范

> 目的：定义 Nebula 前端“观测面板”应如何展示 Gateway/Router 防护指标，形成稳定的观测与调参闭环。

## 1. 职责边界

- 数据层：Gateway/Router 暴露 `/metrics`，xtrace/观测后端负责采集与查询。
- 展示层：前端负责面板 UI（卡片、趋势图、告警态、表格）。
- 本文仅定义展示口径，不绑定具体图表库实现。

---

## 2. 面板目标

1. 快速识别可用性风险（5xx、过载、熔断频繁打开）。
2. 快速识别防护是否有效（重试成功率、候选缩减命中）。
3. 支持参数调优前后对比（按时间窗观察趋势变化）。

---

## 3. 最小指标集（MVP）

## 3.1 Router 侧

- 吞吐与状态：
  - `nebula_router_requests_total`
  - `nebula_router_responses_2xx`
  - `nebula_router_responses_4xx`
  - `nebula_router_responses_5xx`
- 重试与错误分类：
  - `nebula_router_retry_total`
  - `nebula_router_retry_success_total`
  - `nebula_router_upstream_error_total{kind}`
- 防护关键：
  - `nebula_router_request_too_large_total`
  - `nebula_router_route_circuit_skipped_total`
  - `nebula_router_circuit_open_total`
- 延迟：
  - `nebula_route_latency_seconds`（histogram）
  - `nebula_route_ttft_seconds`（histogram）

## 3.2 Gateway 侧

- 吞吐与状态：
  - `nebula_gateway_requests_total`
  - `nebula_gateway_responses_2xx`
  - `nebula_gateway_responses_4xx`
  - `nebula_gateway_responses_5xx`
- 防护：
  - `nebula_gateway_request_too_large_total`
  - `nebula_gateway_upstream_error_total{kind}`

---

## 4. 前端面板布局建议

## 4.1 概览卡片（顶部）

1. 总请求速率（RPS）
2. 5xx 比例（Router + Gateway）
3. 重试成功率（Router）
4. 熔断开启次数（Router）

## 4.2 趋势图（中部）

1. 请求量与状态码趋势（2xx/4xx/5xx）
2. 重试总量与成功量趋势
3. 上游错误分类趋势（connect/timeout/upstream_5xx/other）
4. 路由延迟 P50/P95/P99 趋势
5. TTFT P50/P95 趋势

## 4.3 诊断区（底部）

1. 熔断与候选缩减命中统计（最近 5/15/60 分钟）
2. 超大请求拦截统计（413）
3. 最近高错误窗口（按时间段 Top N）

---

## 5. 关键衍生指标（前端计算）

1. **5xx 比例**
   - `responses_5xx / requests_total`
2. **重试成功率**
   - `retry_success_total / retry_total`
3. **上游错误占比（按 kind）**
   - `upstream_error_total{kind} / upstream_error_total{all}`
4. **熔断影响率**
   - `route_circuit_skipped_total / requests_total`

---

## 6. 告警阈值建议（初始值）

> 以下阈值用于初始上线，需按实际流量校准。

1. 5xx 比例 > 2% 持续 5 分钟
2. 重试成功率 < 20% 持续 10 分钟
3. circuit_open_total 在 5 分钟内突增（超过近 1h 均值 3 倍）
4. timeout 类错误占比 > 40% 持续 10 分钟

---

## 7. 面板验收标准

1. 支持 5m/15m/1h 时间窗切换。
2. 每个图表可下钻到原始指标与标签维度。
3. 能在一次故障演练中明确回答：
   - 错误是哪里开始增长的？
   - 重试是否在起作用？
   - 熔断是否生效并降低坏副本命中？

---

## 8. 后续扩展

1. 按 `model_uid` 维度展示路由质量与错误分布。
2. 按 `tenant/session` 维度做流量与错误隔离视图（如有权限模型支持）。
3. 将参数变更事件（如重试参数、熔断阈值）叠加到趋势图，便于因果分析。
