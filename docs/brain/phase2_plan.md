# Implementation Plan - Phase 2: Dynamic Control

## Goal
Enable dynamic loading and unloading of models via an HTTP API, removing the need for static node configuration.

## Proposed Changes

### 1. `nebula-common`
*   Define `ModelLoadRequest` struct.
*   Define `ModelRequestStatus` enum (Pending, Scheduled, Failed).

### 2. `nebula-gateway`
*   Add `POST /v1/admin/models/load` endpoint.
*   This endpoint will:
    1.  Accept a `ModelLoadRequest` JSON.
    2.  Write a "Model Intent" to Etcd (e.g., `/model_requests/<model_uid>`).
    3.  Return a tracking ID.

### 3. `nebula-scheduler`
*   Watch `/model_requests/` prefix in Etcd.
*   When a new request appears:
    1.  Find a suitable node (currently basic round-robin or first available).
    2.  Generate a `PlacementPlan` for that node.
    3.  Write the `PlacementPlan` to Etcd (which the Node already watches!).

### 4. `nebula-node` (Validation)
*   The node already watches `PlacementPlan`. As long as the scheduler writes a valid plan, the node *should* pick it up and restart vLLM with the new model.
*   *Note*: The current Node implementation restarts the child process if the plan changes. We need to verify this behavior works smoothly for a "new" model.

## Verification
1.  Start the stack with *no* model pre-configured (or a dummy one).
2.  Call `POST /v1/admin/models/load` with `Qwen/Qwen2.5-0.5B-Instruct`.
3.  Observe:
    *   Gateway writes request to Etcd.
    *   Scheduler picks it up and writes Placement.
    *   Node sees Placement and starts vLLM.
    *   Gateway/Router eventually see the new endpoint healthy.
