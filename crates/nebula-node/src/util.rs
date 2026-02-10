use std::time::{SystemTime, UNIX_EPOCH};

use tokio::process::Child;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub async fn stop_child(child: &mut Child) {
    let _ = child.kill().await;
}
