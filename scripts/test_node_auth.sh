export VIRTUAL_ENGINE_TARGET="https://api.deepseek.com"
export VIRTUAL_ENGINE_KEY="sk-c5c3c20154b144bbac50e49f026a020e"
RUST_LOG=info cargo run -p nebula-node --release -- --etcd-endpoint http://127.0.0.1:2379 --node-id local-node --vllm-model-dir /tmp/nebula/model_cache &
