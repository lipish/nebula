use std::pin::Pin;
use std::time::Duration;

use futures_core::Stream;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::{iter, wrappers::ReceiverStream};

pub type EngineDeltaStream = Pin<Box<dyn Stream<Item = String> + Send>>;

pub trait EngineClient: Send + Sync {
    fn stream_text(&self, input: String) -> EngineDeltaStream;
}

#[derive(Debug, Clone)]
pub struct OpenAIEngineClient {
    base_url: String,
    model: String,
    http: reqwest::Client,
}

impl OpenAIEngineClient {
    pub fn new(base_url: String, model: String) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(300))
            .build()
            .expect("reqwest client");

        Self {
            base_url,
            model,
            http,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct StubEngineClient {}

impl StubEngineClient {
    pub fn new() -> Self {
        Self {}
    }

    fn chunk_text(s: &str, chunk_size: usize) -> Vec<String> {
        if s.is_empty() {
            return vec![];
        }
        let mut out = Vec::new();
        let mut buf = String::new();
        for ch in s.chars() {
            buf.push(ch);
            if buf.chars().count() >= chunk_size {
                out.push(std::mem::take(&mut buf));
            }
        }
        if !buf.is_empty() {
            out.push(buf);
        }
        out
    }
}

impl EngineClient for StubEngineClient {
    fn stream_text(&self, input: String) -> EngineDeltaStream {
        let chunks = Self::chunk_text(&input, 16);
        let s = iter(chunks).map(|s| s);
        Box::pin(s)
    }
}

impl EngineClient for OpenAIEngineClient {
    fn stream_text(&self, input: String) -> EngineDeltaStream {
        let (tx, rx) = mpsc::channel::<String>(256);
        let http = self.http.clone();
        let base = self.base_url.trim_end_matches('/').to_string();
        let model = self.model.clone();

        tokio::spawn(async move {
            let url = format!("{base}/v1/chat/completions");
            let body = serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": input}],
                "stream": true,
                "max_tokens": 512,
            });

            let resp = match http.post(url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error=%e, "engine request failed");
                    return;
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                tracing::error!(%status, body=%text, "engine returned error");
                return;
            }

            let mut buf = String::new();
            let mut stream = resp.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = match item {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!(error=%e, "engine stream read failed");
                        return;
                    }
                };

                let s = String::from_utf8_lossy(&chunk);
                buf.push_str(&s);

                while let Some(pos) = buf.find('\n') {
                    let mut line = buf[..pos].to_string();
                    buf.drain(..=pos);

                    line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if !line.starts_with("data:") {
                        continue;
                    }

                    let data = line.trim_start_matches("data:").trim();
                    if data == "[DONE]" {
                        return;
                    }

                    let v: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta = v
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c0| {
                            c0.get("delta")
                                .and_then(|d| d.get("content"))
                                .and_then(|t| t.as_str())
                                .or_else(|| c0.get("text").and_then(|t| t.as_str()))
                        })
                        .unwrap_or("");

                    if !delta.is_empty() {
                        let _ = tx.send(delta.to_string()).await;
                    }
                }
            }
        });

        Box::pin(ReceiverStream::new(rx))
    }
}
