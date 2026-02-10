pub async fn read_engine_env_file(path: &str) -> Option<(String, String)> {
    let content = tokio::fs::read_to_string(path).await.ok()?;
    let mut base_url: Option<String> = None;
    let mut model: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        let k = k.trim();
        let v = v.trim();
        match k {
            "NEBULA_ENGINE_BASE_URL" => base_url = Some(v.to_string()),
            "NEBULA_ENGINE_MODEL" => model = Some(v.to_string()),
            _ => {}
        }
    }
    Some((base_url?, model?))
}
