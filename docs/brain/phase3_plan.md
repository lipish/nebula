# Phase 3 Implementation Plan: Advanced Features & Multi-Model Support

## Goal
Enable the Nebula system to serve multiple models simultaneously on a single node (e.g., Chat + Embeddings) and support LoRA adapters for efficient fine-tuning serving.

## Planned Features

### 1. Multi-Model Node Architecture
*   **Current State**: `nebula-node` processes a single `PlacementPlan` and runs one vLLM instance.
*   **Target State**: `nebula-node` can manage **multiple** concurrent vLLM processes.
*   **Implementation**:
    *   Update `PlacementPlan` watcher to handle multiple assignments for the same node.
    *   Refactor `nebula-node` main loop to maintain a map of `running_processes`.
    *   Ensure distinct ports/log-files for each model.

### 2. Embeddings API Support
*   **Current State**: `/v1/embeddings` is not implemented.
*   **Target State**: Full support for OpenAI-compatible embeddings.
*   **Implementation**:
    *   **Gateway**: Add `POST /v1/embeddings` endpoint.
    *   **Router**: Logic to route embedding requests to appropriate models.
    *   **Node**: Support launching vLLM in embedding mode (if utilizing vLLM for embeddings) or support separate embedding engine container. *Note: vLLM supports embeddings.*

### 3. LoRA Adapter Support
*   **Current State**: Only base models supported.
*   **Target State**: Support dynamic LoRA loading via API.
*   **Implementation**:
    *   Update `ModelLoadRequest` to include `adapters` (list of LoRA paths/names).
    *   Update `nebula-node` to launch vLLM with `--enable-lora`.
    *   Verify checking out specific LoRA modules via the Chat completion API (`model` parameter can point to a LoRA).

## Execution Steps

1.  **Refactor Node for Multi-Process** [High Priority]
    *   Allow Node to reconcile *multiple* assignments.
    *   Dynamic port allocation or assignment-based ports.
2.  **Implement Embeddings** [Medium Priority]
    *   Update Gateway/Router.
    *   Test loading an embedding model (e.g., `bge-m3`) alongside a chat model.
3.  **Implement LoRA** [Low Priority]
    *   Test LoRA configurations.

## Verification
*   Load `Qwen2.5-0.5B` (Chat) AND `bge-small-en` (Embeddings) on the same GPU.
*   Send concurrent Chat and Embedding requests.
