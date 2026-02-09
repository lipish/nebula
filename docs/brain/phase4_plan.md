# Implement Nebula CLI & Management Experience

Phase 4 focuses on making Nebula manageable through a command-line interface, replacing manual `curl` calls with ergonomic commands. This requires expanding the Gateway's Admin API and implementing the `nebula-cli` crate.

## Proposed Changes

### [nebula-gateway]

#### [MODIFY] [main.rs](file:///Users/mac-m4/github/nebula/crates/nebula-gateway/src/main.rs)
- Add administrative endpoints for cluster visibility:
    - `GET /v1/admin/cluster/status`: Aggregated view of nodes, endpoints, and placements.
    - `GET /v1/admin/models/requests`: List current and past model requests.
    - `DELETE /v1/admin/models/requests/:id`: Remove a model request (triggering unload).
- Implement handler functions using `EtcdMetaStore::list_prefix`.

### [nebula-cli]

#### [MODIFY] [main.rs](file:///Users/mac-m4/github/nebula/crates/nebula-cli/src/main.rs) [NEW]
- Implement subcommands using `clap`:
    - `cluster status`: Show nodes and their health.
    - `model list`: Show all requested models and their status (Pending/Scheduled/Ready).
    - `model load <name> --uid <uid> --replicas <n>`: Trigger model deployment.
    - `model unload <id>`: Terminate a model instance.
    - `chat <model_uid>`: (Optional) Simple interactive chat interface.
- Use `reqwest` to interact with `nebula-gateway`.

### [nebula-common]

#### [MODIFY] [model_request.rs](file:///Users/mac-m4/github/nebula/crates/nebula-common/src/model_request.rs)
- Ensure all necessary types for cluster status are defined and serializable.

---

## Verification Plan

### Automated Tests
- I will run shell scripts that use the new `nebula-cli` to:
    1. List an empty cluster.
    2. Load a model.
    3. Monitor the status until "Ready".
    4. Unload the model.

### Manual Verification
1. **Ergonomics**: Verify that `nebula model list` provides a clean, readable table.
2. **End-to-End**: Ensure `nebula model unload` correctly deletes keys in Etcd, causing `nebula-node` to stop the vLLM process.
