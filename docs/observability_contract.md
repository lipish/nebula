# Nebula 可观测性实施规范

## 概述
本规范界定了 Nebula 作为领域控制面与 xtrace 作为通用可观测性底座的责任边界，确立了 Nebula 在推理性能、调度决策及引擎状态方面的观测落地策略。

## 责任边界 (Mechanism vs. Policy)
- **xtrace (底座)**：提供标准上下文传播契约、Trace/Span/Event 原语、Metrics 标准存储、通用 Metadata 查询。
- **Nebula (策略)**：
    - **领域适配**：组件间 Context 透传、引擎 Shim 层埋点。
    - **策略决策**：定义流式响应聚合规则 (TTFT/TPOT)、写入频率。
    - **语义诊断**：定义 `nebula.*` 风格的领域信号命名规范、故障解释规则与诊断 UI。

## 可观测性实施指南

### 1. 领域信号规范 (Namespace: `nebula.*`)
Nebula 所有自定义指标与 Metadata 必须统一命名空间，避免污染通用观测底座。
- **调度信号**：`nebula.scheduling.decision`, `nebula.scheduling.placement_version`
- **节点信号**：`nebula.node.reconcile_status`, `nebula.node.engine_swap`
- **引擎指标**：`nebula.engine.kv_cache_utilization`, `nebula.engine.gpu_memory_usage`

### 2. 推理性能观测 (Streaming Life-cycle)
- **事件上报原语**：使用 xtrace 标准 event 原语记录推理关键阶段。
    - `stream_start`: 记录 TTFT (Time to First Token)。
    - `stream_progress`: 周期性记录 Token 生成进度或延迟。
    - `stream_end`: 记录总延迟与总 Token 数。
- **策略落实**：由 Nebula 侧的 `nebula-observe` 组件负责决定是否逐 Token 上报或按时间窗口聚合，避免高频写入导致的后端压力。

### 3. 上下文传播 (Context Propagation)
- **实现模式**：Nebula 自行负责 Header 注入与提取逻辑。
- **传递链**：`Gateway` (注入/生成) -> `Router` (透传) -> `Node` (提取) -> `Engine Shim` (异步上报)。
- **规范**：严格遵循 xtrace 提供的标准 `TraceID` 与 `SpanContext` HTTP Header 格式。

### 4. 故障诊断解释层
- **根因分析**：诊断规则、阈值告警与根因逻辑封装在 Nebula 控制面内部或独立 Dashboard 定义中。
- **数据关联**：通过 Metadata 关联 `placement_version` 与对应的 `engine_swap` 时间点，在 Nebula UI 实现关联查询。

## 原则
1. **领域逻辑归 Nebula**：任何涉及推理引擎特性的逻辑严禁注入 `xtrace` 核心。
2. **标准对齐**：所有上报数据必须通过 `xtrace` 提供的 Client SDK 及其标准 Contract 接口。
3. **性能优先**：Nebula 侧在写入 Metrics/Events 时，应评估其对推理延迟的影响，必要时采用异步队列上报。
