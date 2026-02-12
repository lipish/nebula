## Engine Stats Pipeline 是什么

简单说，就是**把 vLLM 引擎内部的运行指标，从引擎进程里"搬运"到 Nebula 控制面中**，让 Router 和 Scheduler 能基于真实数据做决策。

### 当前的问题

现在 Nebula 的数据流是这样的：

```
vLLM 引擎进程
  └── 内部有丰富的指标（KV cache 使用率、prefix cache 命中率、正在处理的请求数...）
  └── 通过 GET /metrics 暴露为 Prometheus 格式
  └── 但是没有人来读它 ❌

Node Daemon
  └── heartbeat 只上报 GPU 显存（nvidia-smi）
  └── 不知道引擎内部状态

Router
  └── 路由决策只看 pending_requests（本地计数器，不是引擎真实值）
  └── EndpointStats 里的 kv_cache_used_bytes、prefix_cache_hit_rate 全是空的

Scheduler
  └── 只看 GPU 显存决定放置
  └── 不知道引擎是否过载
```

**引擎有数据，但控制面看不到。** 这就是"缺数据"的本质。

### Pipeline 做什么

建立一条从引擎到控制面的数据管道：

```
vLLM /metrics ──scrape──→ Node Daemon ──etcd──→ Router / Scheduler
                                                    ↓
                                              路由决策 / 扩缩容决策
```

具体三步：

**① Node 侧采集（scrape）**

Node Daemon 的 heartbeat 循环（每 3 秒一次）中，对每个正在运行的引擎发一个 `GET {base_url}/metrics`，拿到 Prometheus 文本格式的指标，解析出关键字段：

| vLLM 指标名 | 含义 | 对应 EndpointStats 字段 |
|---|---|---|
| `vllm:num_requests_waiting` | 排队中的请求数 | `pending_requests` |
| `vllm:gpu_cache_usage_perc` | KV cache 使用百分比 | `kv_cache_used_bytes` / `kv_cache_free_bytes` |
| `vllm:prefix_cache_hit_rate` | 前缀缓存命中率 | `prefix_cache_hit_rate` |

**② 写入 etcd（传输）**

采集到的数据填充到已有的 `EndpointStats` 结构体中，序列化后写入 etcd 的 `/stats/{model_uid}/{replica_id}` 路径。这样数据就进入了 Nebula 的元数据层。

**③ Router 侧同步（消费）**

Router 的 sync 模块（`sync.rs`）已经在 watch `/endpoints/` 前缀了，只需增加一个 watch `/stats/` 前缀的逻辑，把数据同步到 `Router.stats` 这个 DashMap 中。这样路由决策时就能读到真实的引擎状态。

### 有了这条管道之后能做什么

| 消费方 | 能做的事 |
|--------|---------|
| **Router** | LeastKvCache 路由（选 KV cache 最空闲的 endpoint）、PrefixCacheAware 路由（选缓存命中率最高的）、Admission Control（全部过载时拒绝请求） |
| **Scheduler** | 检测引擎是否真正健康（不只看心跳）、基于负载决定是否扩缩容 |
| **前端/CLI** | 展示每个模型实例的真实运行状态（不只是"Ready"） |

这就是为什么说它是"一切优化的前提"——**没有数据，策略再好也是盲人摸象**。