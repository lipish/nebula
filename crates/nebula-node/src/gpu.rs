use tokio::process::Command;

use nebula_common::GpuStatus;

pub async fn read_gpu_statuses() -> Vec<GpuStatus> {
    let output = Command::new("nvidia-smi")
        .arg("--query-gpu=memory.total,memory.used,temperature.gpu,utilization.gpu")
        .arg("--format=csv,noheader,nounits")
        .output()
        .await;

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for (idx, line) in stdout.lines().enumerate() {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 2 {
            continue;
        }
        let total = parts[0].parse::<u64>().unwrap_or(0);
        let used = parts[1].parse::<u64>().unwrap_or(0);
        let temperature = parts.get(2).and_then(|s| s.parse::<u32>().ok());
        let utilization = parts.get(3).and_then(|s| s.parse::<u32>().ok());
        out.push(GpuStatus {
            index: idx as u32,
            memory_total_mb: total,
            memory_used_mb: used,
            temperature_c: temperature,
            utilization_gpu: utilization,
        });
    }
    out
}
