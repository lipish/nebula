# Nebula HA 规划（待完善）

> 状态：规划中（暂不改代码）
> 更新时间：2026-02-15

## 1. 目标

在多机集群场景下，实现 Nebula 控制面与接入面的高可用（HA），并保证 GPU 执行层故障可隔离、可恢复。

核心目标：
- 消除关键单点（SPOF）
- 组件故障自动切换
- 控制面具备可恢复与可扩展能力
- 对业务请求可观测、可演练、可回滚

---

## 2. 当前状态（基于现有部署）

当前更接近“可恢复 + 可扩展基础”，尚未达到完整生产级 HA。

现状判断：
- 单实例组件（存在单点）：`etcd`、`postgres`、`scheduler`、`router`（当前 compose 形态）
- 可横向潜力组件：`gateway`、`bff`（可部署到无 GPU 节点）
- 执行层：`node` 适合部署到有 GPU 节点，天然可多节点扩展

结论：
- 支持多机部署能力基础 ✅
- 完整 HA 闭环（多副本 + 选主 + 自动切换）❌

---

## 3. 组件 HA 角色划分

### 3.1 无 GPU 节点（控制面 / 接入面）
- `gateway`：可多副本，放在 LB 后
- `bff`：可多副本，放在 LB 后
- `router`：建议多副本
- `scheduler`：建议 2~3 副本 + leader election

### 3.2 有 GPU 节点（执行面）
- `node`：多副本（按 GPU 机器数扩展）
- 推理引擎实例：由 node reconcile 拉起

---

## 4. 最小 HA 落地顺序（建议）

### Phase 1：控制面去单点（最高优先级）
1. etcd 升级为 3 节点集群（奇数副本）
2. 所有组件切换到 etcd 集群端点
3. 验证任一 etcd 节点故障不影响读写

### Phase 2：接入层高可用
1. `gateway` 至少 2 副本
2. `bff` 至少 2 副本
3. 前置 LB（Nginx/HAProxy/云 LB）
4. 启用健康检查与失效摘除

### Phase 3：调度层选主
1. `scheduler` 运行 2~3 副本
2. 仅 leader 生效（lease + election）
3. follower 热备，leader 故障自动接管

### Phase 4：数据层高可用
1. `postgres` 主备（或托管 HA）
2. 明确故障切换策略与连接重试策略

### Phase 5：全链路演练
1. 杀接入层副本
2. 杀 scheduler leader
3. 下线单台 GPU 节点
4. 验证请求成功率、恢复时间（RTO）

---

## 5. 建议拓扑（逻辑）

- 接入层（无 GPU）：`LB -> gateway(2+) -> bff(2+)`
- 控制层（无 GPU）：`router(2+)`, `scheduler(2~3, leader/follower)`
- 元数据层：`etcd(3)`，`postgres(HA)`
- 执行层（有 GPU）：`node(N)` + engine instances

---

## 6. 关键设计点（后续实现关注）

- Leader Election
  - scheduler 必须有明确的 leader lease 与抢占/续约机制
- 幂等 Reconcile
  - 重复事件与重试不能引入重复部署/错误回滚
- 版本一致性
  - placement plan 版本与 endpoint 注册版本需严格校验
- 健康探针
  - liveness / readiness 分离
- 重试与超时策略
  - 明确组件间调用超时、退避、熔断策略

---

## 7. 验收标准（Definition of Done）

满足以下条件可认为“达到 HA 基线”：
- 任一单节点故障不导致整体不可用
- gateway/bff 任一副本故障不影响对外 API 可用
- scheduler leader 故障后在目标窗口内自动切换
- 单台 GPU 节点离线时仅影响该节点上的副本，不影响整个系统
- 故障期间关键指标可观测（成功率、延迟、错误码、恢复时间）

---

## 8. 后续任务建议（可拆工单）

1. etcd 3 节点部署与迁移方案
2. gateway/bff 多副本 + LB 健康检查配置
3. scheduler 选主机制设计与实现
4. postgres HA 方案选型（自建或托管）
5. 故障演练脚本与验收报告模板

---

## 9. 备注

本文件为 HA 能力建设的规划基线；后续每个 phase 落地后，应更新：
- 实施状态
- 风险与回滚方案
- 验收结果
