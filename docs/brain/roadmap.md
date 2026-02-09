# Nebula Project Roadmap & Gap Analysis

## 1. Current Status (As of Feb 2026)

The Nebula project has achieved a **functional MVP** status. The core control plane (Rust) and execution plane (vLLM) are integrated and capable of serving LLM chat traffic.

### ✅ Achieved Capabilities
- **Core Architecture**: Successfully separated Control Plane (Rust) and Execution Plane (Python/vLLM).
- **Deployment**: Verified deployment on remote GPU hardware with custom model mirrors (ModelScope).
- **Routing**: Basic request routing via Gateway -> Router -> Node.
- **Resilience**: Node heartbeat mechanics and Etcd registration are functional.
- **Streaming**: Token streaming for Chat Completions is working.

## 2. Critical Gaps ("What needs to be supplemented")

To move from MVP to a production-ready system (and effectively replace Xinference), the following areas require immediate attention:

### A. Lifecycle Management (Highest Priority)
*   **Problem**: Models are currently "hard-coded" into the `nebula-node` startup command. There is no API to dynamically load/unload models.
*   **Gap**: Missing an "Admin API" to request model loading. The Scheduler currently reads static config/etcd rather than reacting to user intent for model scaling.
*   **Goal**: `POST /v1/models/load` -> Scheduler assigns to Node -> Node downloads & starts engine.

### B. Observability & Debugging
*   **Problem**: Logs are scattered across multiple `nohup` files (`gateway.log`, `router.log`, `node.log`).
*   **Gap**: No centralized logging or metrics collection.
*   **Goal**: Integrate `tracing-opentelemetry` or a simple log aggregator. Expose Prometheus metrics for Request Latency, Queue Depth, and GPU Utilization.

### C. Deployment & Operations
*   **Problem**: Deployment requires manual execution of 4+ binaries with complex CLI arguments.
*   **Gap**: Lack of a supervisor (Systemd/Supervisor) or orchestration (Kubernetes/Docker Compose) for the control plane.
*   **Goal**: Create a `nebula-up` CLI or `docker-compose.yml` for one-click deployment.

### D. API Feature Parity
*   **Problem**: Only `/v1/chat/completions` is fully verified.
*   **Gap**:
    - **Embeddings**: Endpoint exists but is marked Not Implemented or experimental.
    - **Function Calling**: Gateway proxy logic needs to ensure tool definitions are correctly passed.
    - **Rerank**: Completely missing.
*   **Goal**: Implement comprehensive OpenAI-compatible endpoints.

## 3. Proposed Roadmap

### Phase 1: Operational Hardening (Next 2 Weeks)
- [ ] **Unified Launcher**: Create a helper script (`bin/nebula-all`) to start Etcd + Gateway + Router + Scheduler with reasonable defaults.
- [ ] **Log Aggregation**: Standardize log formats and maybe stream to a centralized view.
- [ ] **Monitoring**: Add `/metrics` endpoint to Gateway aggregating stats from Router.

### Phase 2: Dynamic Control (Month 1)
- [ ] **Admin API**: Implement `POST /v1/admin/models` in Gateway.
- [ ] **Dynamic Scheduling**: Update Scheduler to watch for "Model Requests" and assign them to available nodes dynamically.
- [ ] **Model Downloader Service**: Decouple model downloading from the Node startup, allowing pre-fetching.

### Phase 4: CLI & Management Experience (Next Steps)
- [ ] **Nebula CLI**: Develop a comprehensive CLI for model management (`list`, `load`, `unload`).
- [ ] **Status Dashboard (Terminal)**: A `nebula status` command showing real-time node/GPU health.
- [ ] **Management API Hardening**: Ensure the Gateway dynamic routes and admin APIs are secure and production-ready.
