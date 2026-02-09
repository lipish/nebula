# Optimization Roadmap

This roadmap outlines pragmatic, high-impact improvements for Nebula, moving it from MVP to a production-ready system.

## Phase 1: Robustness & Observability (High Priority)

Goal: Ensure the system can handle failures gracefully and provides visibility into its internal state.

-   **Node Supervision Tree**:
    -   *Current*: Simple process spawning.
    -   *Optimization*: Implement a supervision tree for the vLLM process. If it crashes, restart it with backoff.
-   **Enhanced Health Checks**:
    -   *Current*: Basic TCP connect / process existence.
    -   *Optimization*: Use `vllm`'s `/health` endpoint to verify inference capability, not just process liveness.
-   **Unified Logging**:
    -   *Current*: Scattered stdout/stderr.
    -   *Optimization*: Aggregate logs from Gateway, Router, and Nodes (via `tracing` subscriber) to a central sink (or at least structured file output).

## Phase 2: Functional Completeness (Medium Priority)

Goal: Close the feature gap with standard OpenAI-compatible proxies.

-   **Implement `/v1/models`**:
    -   *Current*: Missing / Router returns static list?.
    -   *Optimization*: Aggregation endpoint in Gateway that queries available models from Router/Etcd.
-   **Embeddings Support**:
    -   *Current*: 501 Not Implemented.
    -   *Optimization*: Implement proxy logic for `/v1/embeddings` similar to `/v1/chat/completions`. Even if it's just a pass-through to a specific model, it unblocks RAG use cases.

## Phase 3: Intelligent Scheduling (Long Term)

Goal: Move beyond "IdleFirst" to optimize for throughput and latency.

-   **Load-Aware Scheduling**:
    -   *Current*: `IdleFirst` (random/round-robin equivalent if all idle).
    -   *Optimization*: Node reports real-time `kv_cache_usage` and `gpu_utilization` to etcd. Scheduler uses this to place models on least-loaded nodes.
-   **Session Affinity (Sticky Routing)**:
    -   *Current*: Basic implementation exists.
    -   *Optimization*: Verify and harden session affinity for caching benefits (Prefix Caching).
