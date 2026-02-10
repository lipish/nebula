# 开发计划

## 1. 现状概览
- 控制面与执行面链路已打通：动态模型加载、调度、Node 多进程多模型、Embeddings、CLI 基础管理能力（list/load/unload/status）。
- 仍缺：鉴权与 RBAC、可观测性基线（统一日志/指标/追踪）、容量感知调度、多 GPU 自动管理、一键部署、Web Console/后台服务、CLI 完备度（chat/logs/metrics/drain/scale）。

## 2. 目标
- 提供“可观测、可管控、可扩展”的生产级控制面：统一 Control API、鉴权、审计、可观测性、容量感知调度、友好管理体验（CLI + Web）。

## 3. 近期里程碑与优先级
1) **Control API + 鉴权基线**（最高优先级）
   - 定义统一的控制面 API（模型请求、节点/端点状态、操作审计、日志/metrics 拉取）。
   - 为 Admin/Control API 增加鉴权（API key/JWT）与基础 RBAC（viewer/operator/admin），请求限流与审计日志。
2) **可观测性与健康信号**（高优先级）
   - Gateway/Router/Node/Scheduler 暴露 Prometheus 指标；结构化 JSON 日志；trace-id 贯穿。
   - 健康/容量信号：vLLM /health、GPU 显存与利用率、端口占用、plan_version 一致性。
   - CLI 增补 `metrics`、`tail-logs` 便捷查看；为前端预留同一数据源。
3) **CLI 能力补全**（高优先级）
   - 新增：`chat`、`logs`、`metrics`、`drain`（节点/端点下线与迁移）、`scale`（副本调整）、`whoami`/`auth login`。
   - 支持多集群上下文配置、模板化/批量操作。
4) **后台服务 + Web Console MVP**（中优先级）
   - 后台服务：轻量 Rust/Go，封装 Control API，内置 auth/RBAC/审计，供 CLI 与前端共用。
   - Web Console：总览（节点/模型/端点健康）、模型生命周期操作、事件与审计流、基础日志/metrics 视图（可链接 Grafana/Tempo/Loki）。
5) **容量感知与多 GPU 生产化**（中优先级）
   - Node Manager 自动发现多 GPU 并派生子 worker；Scheduler 加入显存/利用率/端口分配器与容量校验；支持模型预拉取/缓存。
6) **运维与交付**（次优先级）
   - 一键部署：docker-compose/helm；升级/回滚脚本；备份/恢复 etcd 元数据流程。
   - 风险防护：重试/回退策略，自愈与 backoff。
7) **功能扩展（可并行）**（低优先级）
   - LoRA 支持（ModelLoadRequest adapters，Node 启动 --enable-lora）。
   - `/v1/models` 聚合、function calling/rerank 等 OpenAI 兼容特性。

## 4. 交付物与验证
- Control API 与鉴权：API 定义文档、RBAC 配置示例、E2E 测试（含拒绝/通过场景）。
- 可观测性：Prometheus 指标清单、采样 dashboards、trace 示例、日志格式样例；健康检查脚本。
- CLI：新增子命令使用示例与回归测试（chat/logs/metrics/drain/scale/auth）。
- 后台服务 & Web Console：MVP 截图/录屏，核心流（登录、总览、模型操作、日志/metrics 查看）的手动/自动化用例。
- 容量感知：调度与 Node 端容量校验单测 + 集成测试（显存不足拒绝、端口分配冲突检测）。

## 5. 风险与缓解
- **安全欠缺**：API 未鉴权可能暴露管理面 → 优先加 token/RBAC 与限流。
- **观测缺失**：无指标/日志会放大排障成本 → 在功能开发并行落地指标与日志。
- **容量误配**：显存/端口冲突导致服务不可用 → 引入容量校验与端口分配器，预检查失败即拒绝调度。
- **多 GPU 复杂度**：自动发现与子进程管理可能引入不稳定 → 先在单机多卡做灰度与回归，再推广多节点。

## 6. 粗略时间顺序
- Week 1-2: Control API + 鉴权，指标/日志基线，CLI auth/metrics/logs。
- Week 3-4: drain/scale、健康信号完善，容量校验与端口分配器；Node 多 GPU 原型。
- Week 5-6: 后台服务与 Web Console MVP；一键部署脚本；观测仪表盘。
- Week 7+: LoRA 与高级 OpenAI 兼容特性，运维回滚/自愈完善。

## 7. Week 1-2 具体任务拆解
- **Control API 定义**：补全模型/端点/节点/审计对象的 protobuf + OpenAPI；统一错误码与幂等语义；补充拒绝/限流场景的返回格式。
- **鉴权与 RBAC**：Gateway/Router 引入 API key/JWT 中间件；meta 存储/下发用户与角色映射；CLI 增加 `auth login`、`whoami`、`--token` 支持；提供示例策略（viewer/operator/admin）。
- **可观测性基线**：各组件暴露指标（请求 QPS/latency、队列长度、调度结果、节点 GPU 内存/利用率）；统一 JSON 日志字段（trace_id/request_id/model/node/endpoint/version）；trace 通过 x-request-id 透传；提供 sample Grafana/Tempo/Loki 配置。
- **CLI 能力**：实现 `metrics`（PromQL 直连或 proxy）、`tail-logs`（Loki/文件流式）、`chat`（走 Gateway）、`logs/metrics` 支持 --follow 与过滤；完善 `status/list` 输出（plan_version、健康信号）。
- **测试与验收**：E2E 用例覆盖鉴权（通过/拒绝）、指标暴露、日志格式、CLI 新子命令；回归基础模型加载/推理；准备最小化 demo 配置用于演示。
