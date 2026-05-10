use serde_json::Value;
use std::path::Path;

pub struct SidecarResult {
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub settings_json: String,
    pub seed: Option<String>,
    pub created_at: Option<String>,
    pub raw_json: String,
}

pub fn find_sidecar(image_path: &Path) -> Option<std::path::PathBuf> {
    // Check for {name}.json (e.g., photo.png -> photo.json)
    let stem = image_path.file_stem()?.to_str()?;
    let parent = image_path.parent()?;
    let sidecar = parent.join(format!("{}.json", stem));
    if sidecar.exists() {
        return Some(sidecar);
    }
    // Check for {name}.{ext}.json (e.g., photo.png.json)
    let full_name = image_path.file_name()?.to_str()?;
    let sidecar2 = parent.join(format!("{}.json", full_name));
    if sidecar2.exists() {
        return Some(sidecar2);
    }
    None
}

pub fn parse_sidecar(sidecar_path: &Path) -> Result<SidecarResult, String> {
    let content = std::fs::read_to_string(sidecar_path)
        .map_err(|e| format!("Failed to read sidecar: {}", e))?;
    let json: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid sidecar JSON: {}", e))?;

    let obj = json.as_object().ok_or("Sidecar is not a JSON object")?;

    let prompt = obj.get("prompt").and_then(|v| v.as_str()).map(String::from);
    let provider = obj.get("provider").and_then(|v| v.as_str()).map(String::from);
    let model = obj.get("model").and_then(|v| v.as_str()).map(String::from);
    let seed = obj.get("seed").and_then(|v| {
        v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(String::from))
    });
    let created_at = obj.get("timestamp").and_then(|v| v.as_str()).map(String::from);

    // Build settings from known fields
    let mut settings = serde_json::Map::new();
    for key in &["quality", "thinking", "n", "platform", "preset", "estimated_cost", "duration_s", "edit_source"] {
        if let Some(val) = obj.get(*key) {
            settings.insert(key.to_string(), val.clone());
        }
    }
    let settings_json = serde_json::to_string(&settings).unwrap_or_else(|_| "{}".to_string());

    Ok(SidecarResult {
        prompt,
        negative_prompt: None,
        provider,
        model,
        settings_json,
        seed,
        created_at,
        raw_json: content,
    })
}
