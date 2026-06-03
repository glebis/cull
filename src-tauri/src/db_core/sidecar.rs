use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

use super::db::Database;
use super::models::GenerationRun;

pub struct SidecarResult {
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub settings_json: String,
    pub seed: Option<String>,
    pub created_at: Option<String>,
    pub raw_json: String,
    pub source_label: Option<String>,
}

pub fn find_sidecar(image_path: &Path) -> Option<PathBuf> {
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
    let json: Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid sidecar JSON: {}", e))?;

    json.as_object().ok_or("Sidecar is not a JSON object")?;

    let mut settings = Map::new();
    collect_known_settings(&json, &mut settings);

    let prompt = first_string(&json, PROMPT_PATHS);
    let mut provider = first_string(&json, PROVIDER_PATHS).map(|p| normalize_provider(&p));
    let mut model = first_string(&json, MODEL_PATHS);
    let mut seed = first_string(&json, SEED_PATHS);
    let mut negative_prompt = first_string(&json, NEGATIVE_PROMPT_PATHS);
    let created_at = first_string(&json, CREATED_AT_PATHS);

    if let Some(prompt_text) = &prompt {
        let parsed = parse_midjourney_parameters(prompt_text);
        if !parsed.settings.is_empty() {
            settings
                .entry("full_prompt".to_string())
                .or_insert_with(|| Value::String(prompt_text.clone()));

            let clean_prompt = strip_midjourney_parameters(prompt_text);
            if clean_prompt != *prompt_text {
                settings
                    .entry("prompt_text".to_string())
                    .or_insert_with(|| Value::String(clean_prompt));
            }

            for (key, value) in parsed.settings {
                settings.entry(key).or_insert(value);
            }
        }
        if seed.is_none() {
            seed = parsed.seed;
        }
        if model.is_none() {
            model = parsed.model;
        }
        if negative_prompt.is_none() {
            negative_prompt = parsed.negative_prompt;
        }
    }

    let looks_midjourney = looks_like_midjourney(&json, &settings);
    if looks_midjourney {
        // Midjourney records carry a bare version (e.g. "6.1" or "config.version");
        // normalize to the canonical "v6.1"/"niji" form. format_midjourney_model is
        // idempotent for already-prefixed values, so applying it to a model resolved
        // from any path is safe.
        model = match model {
            Some(existing) => Some(format_midjourney_model(&existing)),
            None => settings
                .get("version")
                .and_then(string_from_value)
                .map(|v| format_midjourney_model(&v)),
        };
    }
    if provider.is_none() && looks_midjourney {
        provider = Some("midjourney".to_string());
    }

    let source_label = provider
        .as_deref()
        .and_then(source_label_for_provider)
        .or_else(|| {
            first_string(&json, SOURCE_LABEL_PATHS).and_then(|s| source_label_for_provider(&s))
        })
        .or_else(|| {
            if looks_midjourney {
                Some("midjourney".to_string())
            } else {
                None
            }
        });

    let settings_json = serde_json::to_string(&settings).unwrap_or_else(|_| "{}".to_string());

    Ok(SidecarResult {
        prompt,
        negative_prompt,
        provider,
        model,
        settings_json,
        seed,
        created_at,
        raw_json: content,
        source_label,
    })
}

pub fn build_generation_run(
    sidecar: &SidecarResult,
    sidecar_path: &Path,
    source_type: &str,
) -> GenerationRun {
    GenerationRun {
        id: uuid::Uuid::new_v4().to_string(),
        prompt: sidecar.prompt.clone(),
        negative_prompt: sidecar.negative_prompt.clone(),
        provider: sidecar.provider.clone(),
        model: sidecar.model.clone(),
        settings_json: sidecar.settings_json.clone(),
        seed: sidecar.seed.clone(),
        parent_run_id: None,
        source_type: source_type.to_string(),
        source_path: Some(sidecar_path.to_string_lossy().to_string()),
        raw_metadata_json: Some(sidecar.raw_json.clone()),
        created_at: sidecar.created_at.clone(),
        imported_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn link_sidecar_to_image(
    db: &Database,
    image_id: &str,
    image_path: &Path,
    sidecar_path: &Path,
    source_type: &str,
) -> Result<bool, String> {
    let sidecar = parse_sidecar(sidecar_path)?;
    let run = build_generation_run(&sidecar, sidecar_path, source_type);
    let run_id = run.id.clone();

    db.insert_generation_run(&run).map_err(|e| e.to_string())?;
    db.link_image_to_run(image_id, &run_id)
        .map_err(|e| e.to_string())?;
    update_source_from_sidecar(db, image_id, image_path, &sidecar)?;

    Ok(true)
}

const PROMPT_PATHS: &[&[&str]] = &[
    &["prompt"],
    &["full_prompt"],
    &["input_prompt"],
    &["text_prompt"],
    &["job", "prompt"],
    &["image", "prompt"],
    &["metadata", "prompt"],
    &["generation", "prompt"],
    &["midjourney", "prompt"],
];

const NEGATIVE_PROMPT_PATHS: &[&[&str]] = &[
    &["negative_prompt"],
    &["negativePrompt"],
    &["no"],
    &["parameters", "no"],
    &["settings", "no"],
    &["config", "no"],
];

const PROVIDER_PATHS: &[&[&str]] = &[
    &["provider"],
    &["source"],
    &["app"],
    &["platform"],
    &["generator"],
    &["metadata", "provider"],
    &["metadata", "source"],
    &["metadata", "platform"],
];

const SOURCE_LABEL_PATHS: &[&[&str]] = &[
    &["source_label"],
    &["sourceLabel"],
    &["source"],
    &["provider"],
    &["metadata", "source_label"],
    &["metadata", "sourceLabel"],
];

const MODEL_PATHS: &[&[&str]] = &[
    &["model"],
    &["version"],
    &["v"],
    &["parameters", "model"],
    &["parameters", "version"],
    &["settings", "model"],
    &["settings", "version"],
    &["config", "model"],
    &["config", "version"],
    &["metadata", "model"],
];

const SEED_PATHS: &[&[&str]] = &[
    &["seed"],
    &["parameters", "seed"],
    &["settings", "seed"],
    &["config", "seed"],
    &["metadata", "seed"],
];

const CREATED_AT_PATHS: &[&[&str]] = &[
    &["timestamp"],
    &["created_at"],
    &["createdAt"],
    &["created"],
    &["creation_time"],
    &["creationTime"],
    &["metadata", "timestamp"],
    &["metadata", "created_at"],
    &["metadata", "createdAt"],
];

const SETTINGS_CONTAINER_PATHS: &[&[&str]] = &[
    &["settings"],
    &["parameters"],
    &["config"],
    &["metadata"],
    &["job"],
    &["generation"],
    &["midjourney"],
];

#[derive(Default)]
struct ParsedMidjourneyParams {
    settings: Map<String, Value>,
    seed: Option<String>,
    model: Option<String>,
    negative_prompt: Option<String>,
}

fn collect_known_settings(json: &Value, settings: &mut Map<String, Value>) {
    if let Some(obj) = json.as_object() {
        collect_known_settings_from_object(obj, settings);
    }

    for path in SETTINGS_CONTAINER_PATHS {
        if let Some(obj) = get_path(json, path).and_then(Value::as_object) {
            collect_known_settings_from_object(obj, settings);
        }
    }
}

fn collect_known_settings_from_object(obj: &Map<String, Value>, settings: &mut Map<String, Value>) {
    for (key, value) in obj {
        if let Some(canonical) = canonical_setting_key(key) {
            settings.insert(canonical.to_string(), value.clone());
        }
    }
}

fn first_string(json: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| get_path(json, path).and_then(string_from_value))
}

fn get_path<'a>(json: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = json;
    for key in path {
        current = current.as_object()?.get(*key)?;
    }
    Some(current)
}

fn string_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn normalize_provider(provider: &str) -> String {
    let normalized = normalize_key(provider);
    if normalized.contains("midjourney") || normalized == "mj" {
        "midjourney".to_string()
    } else if normalized.contains("openai") {
        "openai".to_string()
    } else if normalized.contains("gemini") || normalized.contains("google") {
        "gemini".to_string()
    } else {
        normalized
    }
}

fn source_label_for_provider(provider: &str) -> Option<String> {
    let normalized = normalize_provider(provider);
    if normalized == "midjourney" {
        Some("midjourney".to_string())
    } else {
        None
    }
}

fn canonical_setting_key(key: &str) -> Option<&'static str> {
    match normalize_key(key).as_str() {
        "ar" | "aspect" | "aspect_ratio" => Some("aspect_ratio"),
        "c" | "chaos" => Some("chaos"),
        "q" | "quality" => Some("quality"),
        "s" | "stylize" | "stylization" => Some("stylize"),
        "w" | "weird" => Some("weird"),
        "v" | "version" => Some("version"),
        "style" => Some("style"),
        "raw" => Some("raw"),
        "niji" => Some("niji"),
        "seed" => Some("seed"),
        "tile" => Some("tile"),
        "stop" => Some("stop"),
        "repeat" | "r" => Some("repeat"),
        "video" => Some("video"),
        "draft" => Some("draft"),
        "turbo" => Some("turbo"),
        "fast" => Some("fast"),
        "relax" => Some("relax"),
        "profile" | "p" | "personalize" => Some("profile"),
        "sref" | "style_reference" => Some("style_reference"),
        "oref" | "omni_reference" => Some("omni_reference"),
        "cref" | "character_reference" => Some("character_reference"),
        "cw" | "character_weight" => Some("character_weight"),
        "iw" | "image_weight" => Some("image_weight"),
        "no" | "negative_prompt" => Some("negative_prompt"),
        "job_id" | "jobid" => Some("job_id"),
        "image_id" | "imageid" => Some("image_id"),
        "platform" => Some("platform"),
        "preset" => Some("preset"),
        "n" => Some("n"),
        "thinking" => Some("thinking"),
        "estimated_cost" => Some("estimated_cost"),
        "duration_s" | "duration" => Some("duration_s"),
        "edit_source" => Some("edit_source"),
        _ => None,
    }
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn parse_midjourney_parameters(prompt: &str) -> ParsedMidjourneyParams {
    let mut parsed = ParsedMidjourneyParams::default();
    let tokens: Vec<&str> = prompt.split_whitespace().collect();
    let mut i = 0;

    while i < tokens.len() {
        let token = tokens[i];
        if !token.starts_with("--") || token.len() <= 2 {
            i += 1;
            continue;
        }

        let (flag, inline_value) = split_flag_token(token);
        let Some(canonical) = canonical_setting_key(&flag) else {
            i += 1;
            continue;
        };

        let value = if flag_takes_value(&flag) {
            match inline_value {
                Some(v) => Some(v),
                None => {
                    let mut values = Vec::new();
                    let mut j = i + 1;
                    while j < tokens.len() && !tokens[j].starts_with("--") {
                        values.push(tokens[j]);
                        j += 1;
                    }
                    if values.is_empty() {
                        None
                    } else {
                        i = j - 1;
                        Some(values.join(" "))
                    }
                }
            }
        } else {
            inline_value
        };

        let json_value = value
            .as_deref()
            .map(scalar_from_str)
            .unwrap_or(Value::Bool(true));
        parsed
            .settings
            .insert(canonical.to_string(), json_value.clone());

        match canonical {
            "seed" => parsed.seed = value.or_else(|| string_from_value(&json_value)),
            "version" => {
                if let Some(v) = value.or_else(|| string_from_value(&json_value)) {
                    parsed.model = Some(format_midjourney_model(&v));
                }
            }
            "niji" => {
                parsed.model = value
                    .filter(|v| !v.is_empty())
                    .map(|v| format!("niji {}", v))
                    .or_else(|| Some("niji".to_string()));
            }
            "negative_prompt" => {
                parsed.negative_prompt = value.or_else(|| string_from_value(&json_value));
            }
            _ => {}
        }

        i += 1;
    }

    parsed
}

fn split_flag_token(token: &str) -> (String, Option<String>) {
    let body = token.trim_start_matches('-');
    for separator in ['=', ':'] {
        if let Some((key, value)) = body.split_once(separator) {
            return (key.to_string(), Some(value.to_string()));
        }
    }
    (body.to_string(), None)
}

fn flag_takes_value(flag: &str) -> bool {
    !matches!(
        normalize_key(flag).as_str(),
        "raw" | "tile" | "video" | "draft" | "turbo" | "fast" | "relax"
    )
}

fn scalar_from_str(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("true") {
        Value::Bool(true)
    } else if trimmed.eq_ignore_ascii_case("false") {
        Value::Bool(false)
    } else if let Ok(n) = trimmed.parse::<i64>() {
        Value::Number(n.into())
    } else if let Ok(n) = trimmed.parse::<f64>() {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(trimmed.to_string()))
    } else {
        Value::String(trimmed.to_string())
    }
}

fn strip_midjourney_parameters(prompt: &str) -> String {
    prompt
        .split_whitespace()
        .take_while(|token| !token.starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_midjourney_model(version: &str) -> String {
    let trimmed = version.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with('v') || lower.starts_with("niji") {
        trimmed.to_string()
    } else {
        format!("v{}", trimmed)
    }
}

fn looks_like_midjourney(json: &Value, settings: &Map<String, Value>) -> bool {
    if first_string(json, PROVIDER_PATHS)
        .map(|s| normalize_provider(&s) == "midjourney")
        .unwrap_or(false)
    {
        return true;
    }

    if first_string(json, SOURCE_LABEL_PATHS)
        .map(|s| normalize_provider(&s) == "midjourney")
        .unwrap_or(false)
    {
        return true;
    }

    [
        "stylize",
        "chaos",
        "weird",
        "style_reference",
        "omni_reference",
        "character_reference",
        "niji",
        "profile",
    ]
    .iter()
    .any(|key| settings.contains_key(*key))
}

fn update_source_from_sidecar(
    db: &Database,
    image_id: &str,
    image_path: &Path,
    sidecar: &SidecarResult,
) -> Result<(), String> {
    let Some(label) = sidecar.source_label.as_deref() else {
        return Ok(());
    };

    let (width, height) = image::open(image_path)
        .map(|img| (img.width(), img.height()))
        .unwrap_or((0, 0));
    let aspect_ratio = width as f64 / height.max(1) as f64;
    let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
        "square"
    } else if aspect_ratio > 1.0 {
        "landscape"
    } else {
        "portrait"
    };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    let evidence = serde_json::json!([{
        "detector": "sidecar",
        "source_label": label,
        "confidence": 0.98,
        "details": format!("Sidecar generation metadata provider: {}", sidecar.provider.as_deref().unwrap_or(label)),
    }])
    .to_string();

    db.update_source_detection(
        image_id,
        Some(label),
        0.98,
        &evidence,
        Some(true),
        sidecar.prompt.as_deref(),
        aspect_ratio,
        orientation,
        megapixels,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_sidecar(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn parses_midjourney_prompt_parameters() {
        let file = write_sidecar(
            r#"{
                "provider": "Midjourney",
                "prompt": "glass observatory above a black fjord --ar 16:9 --stylize 250 --chaos 12 --seed 4242 --v 7 --style raw --tile",
                "job_id": "abc123"
            }"#,
        );

        let parsed = parse_sidecar(file.path()).unwrap();
        let settings: Value = serde_json::from_str(&parsed.settings_json).unwrap();

        assert_eq!(parsed.provider.as_deref(), Some("midjourney"));
        assert_eq!(parsed.source_label.as_deref(), Some("midjourney"));
        assert_eq!(parsed.seed.as_deref(), Some("4242"));
        assert_eq!(parsed.model.as_deref(), Some("v7"));
        assert_eq!(settings["aspect_ratio"], "16:9");
        assert_eq!(settings["stylize"], 250);
        assert_eq!(settings["chaos"], 12);
        assert_eq!(settings["style"], "raw");
        assert_eq!(settings["tile"], true);
        assert_eq!(
            settings["prompt_text"],
            "glass observatory above a black fjord"
        );
    }

    #[test]
    fn parses_midjourney_nested_config_export() {
        let file = write_sidecar(
            r#"{
                "source": "midjourney_web",
                "image": { "prompt": "editorial portrait in sodium light" },
                "config": {
                    "aspect_ratio": "4:5",
                    "version": "6.1",
                    "stylize": 100,
                    "raw": true
                },
                "metadata": {
                    "seed": 918273,
                    "created_at": "2026-05-12T10:00:00Z"
                }
            }"#,
        );

        let parsed = parse_sidecar(file.path()).unwrap();
        let settings: Value = serde_json::from_str(&parsed.settings_json).unwrap();

        assert_eq!(
            parsed.prompt.as_deref(),
            Some("editorial portrait in sodium light")
        );
        assert_eq!(parsed.provider.as_deref(), Some("midjourney"));
        assert_eq!(parsed.source_label.as_deref(), Some("midjourney"));
        assert_eq!(parsed.model.as_deref(), Some("v6.1"));
        assert_eq!(parsed.seed.as_deref(), Some("918273"));
        assert_eq!(parsed.created_at.as_deref(), Some("2026-05-12T10:00:00Z"));
        assert_eq!(settings["aspect_ratio"], "4:5");
        assert_eq!(settings["raw"], true);
    }
}
