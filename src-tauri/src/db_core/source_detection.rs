use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub detector: String,
    pub source_label: Option<String>,
    pub confidence: f64,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDetectionResult {
    pub source_label: Option<String>,
    pub confidence: f64,
    pub is_ai_generated: Option<bool>,
    pub ai_prompt: Option<String>,
    pub evidence: Vec<SourceEvidence>,
}

impl SourceDetectionResult {
    pub fn unknown() -> Self {
        Self {
            source_label: None,
            confidence: 0.0,
            is_ai_generated: None,
            ai_prompt: None,
            evidence: vec![],
        }
    }

    pub fn to_evidence_json(&self) -> String {
        serde_json::to_string(&self.evidence).unwrap_or_else(|_| "[]".to_string())
    }
}
