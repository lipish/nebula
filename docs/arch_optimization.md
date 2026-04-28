# Nebula 架构优化建议

## 总体判断

Nebula 当前的总体方向是合理的：Rust 负责控制面，Python/推理引擎负责执行面，etcd 作为权威状态源，Scheduler 写入期望状态，Node 通过 watch + reconcile 对齐实际状态，Router 基于 endpoint 状态进行请求路由。

主要风险不在大方向，而在部分边界开始重叠，尤其是 Gateway、Router 与 UniGateway 的职责关系尚未收敛。当前工作树中 `nebula-gateway` 也处于集成未闭合状态，已经影响整个 workspace 编译。

## 优先级建议

### P0：先修复 Gateway 编译与集成闭环

当前 `cargo check --workspace` 失败点集中在 `crates/nebula-gateway`：

- `unigateway` 1.7 API 与当前代码不匹配。
- `pool_sync` 模块被调用但未声明或不存在。
- `st` 在定义前被使用。
- `async_trait` 依赖缺失。
- BFF 代理逻辑引用的 header/error helper 函数缺失。

建议先把 Gateway 恢复到可编译状态，再继续推进 UniGateway 集成。否则后续架构讨论会被未完成代码干扰。

### P0：明确 Gateway、Router、UniGateway 边界

当前项目原本已有独立 `nebula-router`，同时 `nebula-gateway` 正在引入 `unigateway`。这会形成潜在的两套路由层。

建议明确三者关系：

- Gateway：对外 OpenAI-compatible HTTP/SSE、鉴权、审计、错误映射、请求上下文提取。
- Router：基于 endpoint、placement、stats 做模型实例选择、重试、熔断、过载保护。
- UniGateway：如果继续引入，应定位为协议执行或高性能转发库，而不是再实现一套集群路由决策。

如果 UniGateway 要替代 `nebula-router`，应显式规划迁移路径；如果只是嵌入 Gateway，则不要让它绕过 Router 的调度与观测语义。

### P1：将 Placement 写入改为 CAS

`docs/architecture.md` 已经规定 Scheduler 更新 placement 必须使用 CAS，`nebula-meta` 也提供了 `compare_and_swap`。但当前 scheduler 中仍存在直接 `put` placement 的路径。

单 Scheduler 场景下风险较低，但未来引入 HA scheduler、并发 reconcile 或自动扩缩容后，直接 `put` 会带来 last-write-wins 风险。

建议：

- 所有 `/placements/{model_uid}` 更新统一通过 `compare_and_swap`。
- 明确 `PlacementPlan.version` 的语义，是逻辑版本还是时间戳。
- 对 CAS 失败路径增加重读、重算和重试策略。

### P1：收敛 Gateway 与 BFF 的 API 职责

当前前端主要访问 BFF 的 `/api` 和 `/api/v2`，Gateway 又代理部分 `/v2` 到 BFF，同时 Gateway 自己也暴露 `/v1/admin`。这会导致鉴权、审计、错误语义和 API 所有权重复。

建议选择一种清晰模式：

- 模式 A：Gateway 只负责推理入口和少量运维只读 API，BFF 负责控制台、用户、模板、资源管理。
- 模式 B：Gateway 作为统一入口，BFF 完全内网化，所有控制台 API 都经 Gateway 转发。

无论选择哪种，都应保证同一类 API 只有一个 owner。

### P1：统一可观测性与鉴权初始化

Gateway、Router、Scheduler、Node 多数使用 `nebula_common::telemetry::init_tracing`，但 BFF 当前直接使用 `tracing_subscriber::fmt()` 初始化。

建议统一服务初始化方式，避免跨服务 trace、日志格式、xtrace token 处理和环境变量行为不一致。

鉴权也应明确区分：

- 推理 API token 鉴权。
- 控制台用户 session 鉴权。
- 服务间内部调用 token。

这三类鉴权不要混用同一套隐式规则。

### P2：拆分过重的 `nebula-common`

`nebula-common` 当前同时包含领域类型、auth、telemetry 等内容。短期方便，但长期会让所有依赖 common 的 crate 被横切基础设施拖重。

建议后续拆分为：

- `nebula-common-types`：纯领域类型，如 placement、endpoint、model request、node status。
- `nebula-common-auth`：鉴权与角色模型。
- `nebula-common-telemetry`：tracing、xtrace、OpenTelemetry 初始化。

拆分不必马上做，但应避免继续把新基础设施逻辑放入 common。

### P2：增强测试与架构回归保护

当前仓库已有部分单元测试，但整体缺少覆盖关键控制面契约的测试。

建议优先补齐：

- Router 策略、熔断、过载保护和 plan_version 过滤测试。
- Scheduler placement CAS 与 reconcile 冲突测试。
- Node watch 断线后 full reconcile 测试。
- Gateway OpenAI-compatible streaming 事件序列测试。
- BFF/Gateway 鉴权边界测试。

这些测试比简单 handler 测试更能保护架构稳定性。

### P2：前端按视图拆包

前端 `npm run build` 可以通过，但主 JS chunk 已超过 500KB。控制台项目短期可接受，后续页面继续增加时建议按视图 dynamic import，降低首屏加载成本。

## 建议执行顺序

1. 修复 `nebula-gateway` 编译问题。
2. 决定 `Gateway / Router / UniGateway` 的职责边界。
3. 将 placement 更新路径改为 CAS。
4. 收敛 Gateway 与 BFF 的 API owner。
5. 统一 telemetry/auth 初始化。
6. 拆分 `nebula-common`，并补关键架构测试。

## 结论

Nebula 的控制面架构方向值得继续推进，但当前最需要避免的是在 Gateway 中不断叠加协议、路由、BFF 代理和控制台职责。只要先把 Gateway/Router/UniGateway 边界固定下来，再补齐 placement 一致性和测试保护，整体架构可以稳定演进。