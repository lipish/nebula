# Implement Embeddings API & Dynamic Routing

This plan covers adding support for the OpenAI-compatible `/v1/embeddings` API and making the `nebula-router` dynamic so it can route requests to different models based on the model name in the request body.

## Proposed Changes

### [nebula-common]

#### [MODIFY] [model_request.rs](file:///Users/mac-m4/github/nebula/crates/nebula-common/src/model_request.rs)
- Define `EmbeddingRequest` and `EmbeddingResponse` structs to facilitate parsing and testing.

### [nebula-router]

#### [MODIFY] [main.rs](file:///Users/mac-m4/github/nebula/crates/nebula-router/src/main.rs)
- Refactor `proxy_chat_completions` to:
    - Attempt to parse the request body as JSON.
    - Extract the `model` field.
    - Use the extracted model name as the `model_uid` for routing.
    - Fall back to the default `--model-uid` if no model is specified in the request.
- This change allows a single router instance to serve multiple distinct models (e.g., Qwen for chat and BGE for embeddings).

### [nebula-gateway]

#### [MODIFY] [main.rs](file:///Users/mac-m4/github/nebula/crates/nebula-gateway/src/main.rs)
- Clean up `proxy_post` special logic for embeddings if possible, or ensure it works well with the new dynamic router.

---

## Verification Plan

### Automated Tests
- I will create a new test script `tests/test_embeddings.py` (or a shell script with `curl`) that:
    1. Loads a chat model.
    2. Loads an embedding model.
    3. Sends a chat request and verifies the response.
    4. Sends an embedding request and verifies the response.

### Manual Verification
1. **Model Loading**:
   - Deploy `qwen2_5_0_5b` as a chat model.
   - Deploy `qwen2_5_0_5b` (or another model) as an embedding model with a unique UID (e.g., `qwen_emb`).
2. **Inference**:
   - `curl -X POST http://127.0.0.1:8081/v1/chat/completions -d '{"model": "qwen2.5-0.5b", ...}'`
   - `curl -X POST http://127.0.0.1:8081/v1/embeddings -d '{"model": "qwen_emb", "input": "test text"}'`
3. **GPU Audit**:
   - Check `nvidia-smi` to ensure both processes are running within their allocated memory limits.
