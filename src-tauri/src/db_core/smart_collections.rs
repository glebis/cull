use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum FilterNode {
    #[serde(rename = "group")]
    Group { op: GroupOp, children: Vec<FilterNode> },
    #[serde(rename = "not")]
    Not { child: Box<FilterNode> },
    #[serde(rename = "rule")]
    Rule { field: Field, op: RuleOp, value: FilterValue },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GroupOp {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Rating,
    ColorLabel,
    Decision,
    Format,
    Width,
    Height,
    AspectRatio,
    Orientation,
    SourceLabel,
    SourceConfidence,
    IsAiGenerated,
    AiPrompt,
    Folder,
    ImportedAt,
    OriginalDate,
    FileSize,
    ClipSimilarTo,
    ClipTextMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuleOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
    In,
    NotIn,
    Between,
    IsEmpty,
    IsNotEmpty,
    LastNDays,
    ThisWeek,
    ThisMonth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FilterValue {
    String(String),
    Number(f64),
    Bool(bool),
    StringArray(Vec<String>),
    Range { from: String, to: String },
    ClipImage { image_id: i64, threshold: Option<f64> },
    ClipText { text: String, threshold: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCollection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub collection_type: String,
    pub filter_json: Option<String>,
    pub nl_query: Option<String>,
    pub is_preset: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub image_count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_node_serialization_roundtrip() {
        let filter = FilterNode::Group {
            op: GroupOp::And,
            children: vec![
                FilterNode::Rule {
                    field: Field::Rating,
                    op: RuleOp::Gte,
                    value: FilterValue::Number(4.0),
                },
                FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("midjourney".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: FilterNode = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, deserialized);
    }

    #[test]
    fn test_nested_not_group() {
        let filter = FilterNode::Group {
            op: GroupOp::And,
            children: vec![
                FilterNode::Rule {
                    field: Field::Rating,
                    op: RuleOp::Gte,
                    value: FilterValue::Number(4.0),
                },
                FilterNode::Not {
                    child: Box::new(FilterNode::Rule {
                        field: Field::SourceLabel,
                        op: RuleOp::Eq,
                        value: FilterValue::String("midjourney".to_string()),
                    }),
                },
            ],
        };

        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: FilterNode = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, deserialized);
    }

    #[test]
    fn test_date_filter_last_n_days() {
        let filter = FilterNode::Rule {
            field: Field::ImportedAt,
            op: RuleOp::LastNDays,
            value: FilterValue::Number(7.0),
        };

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("last_n_days"));
        assert!(json.contains("imported_at"));
    }
}
