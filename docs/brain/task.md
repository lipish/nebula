# Remote Deployment Verification (2x 5090)

- [ ] Verify Remote Access & Hardware
    - [x] Connect via Jump Host and check GPUs (`nvidia-smi`) - **Found only 1 GPU!**
    - [ ] Check for existing `nebula` code or clone/sync it
# Remote Deployment Verification (Single 5090)

- [ ] Prepare Environment (Remote)
    - [x] Install Rust (`rustup`)
    - [x] Install vLLM (`pip`) - **Finalizing installation**
    - [x] Install/Check Etcd - **Installed binary**
    - [x] Sync `nebula` source code to remote
    - [x] Install `protoc` (binary) on remote
    - [x] Build Nebula release on remote
- [ ] Execute Deployment
    - [x] Start Etcd
    - [x] Start Control Plane (Gateway, Router)
    - [x] Start Single Node (GPU0)
- [x] **Refactor Node for Dynamic Switching**
    - [x] Remove hardcoded model in `nebula-node`.
    - [x] Implement Watcher for `PlacementPlan`.
    - [x] Logic to start/stop vLLM process based on plan.
    - [x] Support environment variable injection for ModelScope (`VLLM_USE_MODELSCOPE`).
- [ ] Validate System
    - [x] Schedule model
    - [x] Run test requests
- [x] Remote Access Setup
    - [x] Configure SSH tunnel on `mynotes.fit`
    - [x] Verify local connectivity
- [x] Project Gap Analysis
    - [x] Analyze codebase and deployment state
    - [x] Create Roadmap artifact

- [x] Phase 1: Operational Hardening
    - [x] Create `bin/nebula-up.sh` unified launcher
    - [x] Create `bin/nebula-down.sh` cleanup script
    - [x] Verify scripts on remote server

- [x] Phase 2: Dynamic Control
    - [x] Define `ModelRequest` types in `nebula-common`
    - [x] Implement `POST /v1/admin/models/load` in `nebula-gateway`
    - [x] Update `nebula-scheduler` to handle model requests
    - [x] Verify dynamic model loading (Control Plane verified)

- [x] Phase 3: Advanced Features (Multi-Model & Embeddings)
    - [x] **Multi-Process Node**: Refactor `nebula-node` to support multiple concurrent models.
    - [x] **Scheduler Port Management**: Updated scheduler to assign unique ports.
    - [x] **Embeddings API**: Implement `POST /v1/embeddings` in Gateway/Router.
    - [ ] ~~**LoRA Support**: Enable LoRA adapters in `ModelLoadRequest` and Node.~~ (Deferred)
    - [x] **Verification**: Run Chat + Embeddings simultaneously on single GPU.

- [x] Phase 4: CLI & Management Experience
    - [x] **Admin API Expansion**: Add `list` and `unload` endpoints to Gateway.
    - [x] **Nebula CLI Implementation**: Basic `list`, `load`, `unload` commands.
    - [x] **Cluster Monitor**: `nebula status` for node/GPU/Endpoint overview.
    - [ ] **Interactive Test**: `nebula chat` subcommand for quick model testing.
    - [x] **Final Verification**: Confirm all components build and CLI reports status correctly.
