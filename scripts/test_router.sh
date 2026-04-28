curl -v -H "Content-Type: application/json" -d '{"model":"virtual-deepseek","messages":[{"role":"user","content":"Hello!"}],"stream":false}' http://127.0.0.1:18081/v1/chat/completions
