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
    println!("{}", success_output(json, value));
}

fn success_output(json: bool, value: &Value) -> String {
    if json {
        serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
    } else {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_output_reports_rated_image_in_json_and_human_modes() {
        let value = serde_json::json!({ "status": "ok", "image_id": "img1", "rating": 5 });

        let json = success_output(true, &value);
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), value);
        assert!(!json.contains('\n'));

        let human = success_output(false, &value);
        assert!(human.contains("\"img1\""));
        assert!(human.contains("\"rating\""));
        assert!(human.contains('\n'));
    }

    #[test]
    fn success_output_reports_object_matches_in_json_and_human_modes() {
        let value = serde_json::json!([
            { "image_id": "img-high", "confidence": 0.93 },
            { "image_id": "img-low", "confidence": 0.71 }
        ]);

        let json = success_output(true, &value);
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), value);
        assert!(!json.contains('\n'));

        let human = success_output(false, &value);
        assert!(human.contains("\"img-high\""));
        assert!(human.contains("\"confidence\""));
        assert!(human.contains('\n'));
    }

    #[test]
    fn success_output_reports_similarity_matches_in_json_and_human_modes() {
        let value = serde_json::json!([
            { "image_id": "near", "similarity": 0.94, "model": "clip-vit-b32" },
            { "image_id": "far", "similarity": 0.42, "model": "clip-vit-b32" }
        ]);

        let json = success_output(true, &value);
        assert_eq!(serde_json::from_str::<Value>(&json).unwrap(), value);
        assert!(!json.contains('\n'));

        let human = success_output(false, &value);
        assert!(human.contains("\"near\""));
        assert!(human.contains("\"similarity\""));
        assert!(human.contains("\"clip-vit-b32\""));
        assert!(human.contains('\n'));
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
