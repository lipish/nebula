# UniGateway (UG) 集成模式与架构对齐建议

## 1. 设计哲学 (Design Philosophy)
UG 的核心定位是 **"library-first"**。这意味着 UG 致力于提供极致高性能的协议解析与调度执行能力，而不内置具体的服务发现、配置存储或进程生命周期管理。嵌入者 (Embedder) 应通过实现 `trait` 接口将自身的需求（如集群状态感知、路由决策）与 UG 的执行能力进行**组合 (Composition)**，而非**修改 (Modification)**。

## 2. 核心架构集成模式

| 诉求场景 | 现有架构模式 | Nebula 集成方案 |
| :--- | :--- | :--- |
| **动态状态感知** | 依赖静态 TOML 配置 | **“响应式 PoolHost”**：嵌入者实现 `PoolHost`，通过内部持有 `Arc<DashMap<...>>` 或缓存句柄，将集群中心化存储（如 etcd）的变更实时映射至 `Endpoint` 元数据。 |
| **路由决策外置** | 库内策略计算 | **“显式路由控制”**：调度器在调用 `dispatch` 前，根据集群负载构造单实例的 `HostDispatchTarget::Pool(single_endpoint)`，将调度决策完全外置。 |
| **审计与请求修改** | 只读 Hook | **“扩展 GatewayHooks”**：通过 `on_request` / `on_response` 的 `&mut` 引用，允许嵌入者注入自定义 Header 或进行请求转换。 |

## 3. 针对 UG 的改进建议（非架构重构）

为了更好地支持高性能生产环境，建议进行以下低成本、高性能的优化：

*   **增强 `GatewayHooks` 的灵活性**：
    *   增加 `fn on_request(&self, req: &mut ProxyChatRequest)` 和 `fn on_response(&self, resp: &mut ProtocolHttpResponse)`。
    *   **理由**：满足生产环境（如审计、鉴权透传和 Header 注入）需求，且不会引入额外的运行时开销。
*   **运行时元数据更新机制**：
    *   在 `ProviderPool` 或 `Endpoint` 结构体上支持运行时部分字段的 Upsert 更新能力。
    *   **理由**：支持无需重刷配置即可刷新端点状态（如权重），适应快速变化的分布式环境。
*   **文档落地 (Best Practices)**：
    *   增加 `docs/embedder_patterns.md`，将上述三种集成模式显式记录。
    *   **理由**：帮助开发者理解如何正确地将 UG 嵌入到高性能生产环境中。

## 4. 技术决策说明
*   **拒绝 MiddlewareRegistry**：避免为了追求“灵活性”而引入 `Middleware` 链带来的运行时动态分发开销。高性能场景优先使用静态分发（Trait 方法调用）。
*   **保持 Core 的简洁**：UG 保持作为协议驱动的“纯执行器”。任何集群级逻辑（如中心化存储同步、复杂调度算法）均应作为 Embedded 层实现，由开发者自行负责稳定性。
