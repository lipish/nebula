export VIRTUAL_ENGINE_TARGET="https://api.deepseek.com"
export VIRTUAL_ENGINE_KEY="Bearer testing"
cargo run -p nebula-node --release -- --etcd-endpoint http://127.0.0.1:2379 --node-id local-node --vllm-model-dir /tmp/nebula/model_cache &
