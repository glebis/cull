use serde_json::Value;
use std::path::Path;

pub fn load_params(params_json: Option<&str>, params_file: Option<&Path>) -> Result<Value, String> {
    match (params_json, params_file) {
        (Some(_), Some(_)) => Err("Use params_json or params_file, not both".to_string()),
        (Some(raw), None) => serde_json::from_str(raw).map_err(|e| format!("Invalid JSON: {}", e)),
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
            serde_json::from_str(&raw).map_err(|e| format!("Invalid JSON: {}", e))
        }
        (None, None) => Ok(serde_json::json!({})),
    }
}

pub fn print_success(json: bool, value: &Value) {
    if json {
        println!(
            "{}",
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
        );
    }
}

pub fn print_error(json: bool, message: &str) {
    if json {
        println!(
            "{}",
            serde_json::json!({"event": "error", "message": message}).to_string()
        );
    } else {
        crate::safe_eprintln!("{}", message);
    }
}
