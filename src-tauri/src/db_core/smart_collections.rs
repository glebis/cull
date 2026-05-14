use rusqlite::types::Value as SqlValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum FilterNode {
    #[serde(rename = "group")]
    Group {
        op: GroupOp,
        children: Vec<FilterNode>,
    },
    #[serde(rename = "not")]
    Not { child: Box<FilterNode> },
    #[serde(rename = "rule")]
    Rule {
        field: Field,
        op: RuleOp,
        value: FilterValue,
    },
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
    SearchText,
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
    Range {
        from: String,
        to: String,
    },
    ClipImage {
        image_id: i64,
        threshold: Option<f64>,
    },
    ClipText {
        text: String,
        threshold: f64,
    },
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

impl Field {
    fn to_column(&self) -> &str {
        match self {
            Field::Rating => "s.star_rating",
            Field::ColorLabel => "s.color_label",
            Field::Decision => "s.decision",
            Field::Format => "i.format",
            Field::Width => "i.width",
            Field::Height => "i.height",
            Field::AspectRatio => "i.aspect_ratio",
            Field::Orientation => "i.orientation",
            Field::SourceLabel => "i.source_label",
            Field::SourceConfidence => "i.source_confidence",
            Field::IsAiGenerated => "i.is_ai_generated",
            Field::AiPrompt => "i.ai_prompt",
            Field::SearchText => "search_text",
            Field::Folder => "f.path",
            Field::ImportedAt => "i.imported_at",
            Field::OriginalDate => "i.original_date",
            Field::FileSize => "i.file_size",
            Field::ClipSimilarTo | Field::ClipTextMatch => "unsupported",
        }
    }
}

impl FilterNode {
    pub fn to_sql_clause(&self) -> std::result::Result<(String, Vec<SqlValue>), String> {
        match self {
            FilterNode::Group { op, children } => {
                if children.is_empty() {
                    return Ok(("1=1".to_string(), vec![]));
                }
                let separator = match op {
                    GroupOp::And => " AND ",
                    GroupOp::Or => " OR ",
                };
                let mut parts = vec![];
                let mut all_params = vec![];
                for child in children {
                    let (sql, params) = child.to_sql_clause()?;
                    parts.push(format!("({})", sql));
                    all_params.extend(params);
                }
                Ok((parts.join(separator), all_params))
            }
            FilterNode::Not { child } => {
                let (sql, params) = child.to_sql_clause()?;
                Ok((format!("NOT ({})", sql), params))
            }
            FilterNode::Rule { field, op, value } => {
                if matches!(field, Field::SearchText) {
                    return text_search_clause(op, value);
                }

                let col = field.to_column();
                if col == "unsupported" {
                    return Err(format!(
                        "Field {:?} requires embedding search, not SQL filtering",
                        field
                    ));
                }

                match (op, value) {
                    (RuleOp::Eq, FilterValue::String(v)) => {
                        Ok((format!("{} = ?", col), vec![SqlValue::Text(v.clone())]))
                    }
                    (RuleOp::Eq, FilterValue::Number(v)) => {
                        Ok((format!("{} = ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Eq, FilterValue::Bool(v)) => {
                        Ok((format!("{} = ?", col), vec![SqlValue::Integer(*v as i64)]))
                    }
                    (RuleOp::Neq, FilterValue::String(v)) => Ok((
                        format!("({} IS NULL OR {} != ?)", col, col),
                        vec![SqlValue::Text(v.clone())],
                    )),
                    (RuleOp::Neq, FilterValue::Number(v)) => Ok((
                        format!("({} IS NULL OR {} != ?)", col, col),
                        vec![SqlValue::Real(*v)],
                    )),
                    (RuleOp::Gt, FilterValue::Number(v)) => {
                        Ok((format!("{} > ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Gte, FilterValue::Number(v)) => {
                        Ok((format!("{} >= ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Lt, FilterValue::Number(v)) => {
                        Ok((format!("{} < ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Lte, FilterValue::Number(v)) => {
                        Ok((format!("{} <= ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Contains, FilterValue::String(v)) => Ok((
                        format!("{} LIKE ?", col),
                        vec![SqlValue::Text(format!("%{}%", v))],
                    )),
                    (RuleOp::NotContains, FilterValue::String(v)) => Ok((
                        format!("({} IS NULL OR {} NOT LIKE ?)", col, col),
                        vec![SqlValue::Text(format!("%{}%", v))],
                    )),
                    (RuleOp::In, FilterValue::StringArray(vals)) => {
                        let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
                        let params: Vec<SqlValue> =
                            vals.iter().map(|v| SqlValue::Text(v.clone())).collect();
                        Ok((format!("{} IN ({})", col, placeholders.join(",")), params))
                    }
                    (RuleOp::NotIn, FilterValue::StringArray(vals)) => {
                        let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
                        let params: Vec<SqlValue> =
                            vals.iter().map(|v| SqlValue::Text(v.clone())).collect();
                        Ok((
                            format!(
                                "({} IS NULL OR {} NOT IN ({}))",
                                col,
                                col,
                                placeholders.join(",")
                            ),
                            params,
                        ))
                    }
                    (RuleOp::IsEmpty, _) => {
                        Ok((format!("({} IS NULL OR {} = '')", col, col), vec![]))
                    }
                    (RuleOp::IsNotEmpty, _) => {
                        Ok((format!("({} IS NOT NULL AND {} != '')", col, col), vec![]))
                    }
                    (RuleOp::LastNDays, FilterValue::Number(days)) => Ok((
                        format!("{} >= datetime('now', '-{} days')", col, *days as i64),
                        vec![],
                    )),
                    (RuleOp::ThisWeek, _) => Ok((
                        format!("{} >= datetime('now', 'weekday 0', '-7 days')", col),
                        vec![],
                    )),
                    (RuleOp::ThisMonth, _) => Ok((
                        format!("{} >= datetime('now', 'start of month')", col),
                        vec![],
                    )),
                    (RuleOp::Between, FilterValue::Range { from, to }) => Ok((
                        format!("{} BETWEEN ? AND ?", col),
                        vec![SqlValue::Text(from.clone()), SqlValue::Text(to.clone())],
                    )),
                    _ => Err(format!(
                        "Unsupported operator {:?} for field {:?}",
                        op, field
                    )),
                }
            }
        }
    }
}

fn text_search_clause(
    op: &RuleOp,
    value: &FilterValue,
) -> std::result::Result<(String, Vec<SqlValue>), String> {
    let FilterValue::String(term) = value else {
        return Err("search_text requires a string value".to_string());
    };
    if term.trim().is_empty() {
        return Ok(("1=1".to_string(), vec![]));
    }

    let sql = "(
        f.path LIKE ?
        OR i.ai_prompt LIKE ?
        OR i.raw_metadata LIKE ?
        OR EXISTS (
            SELECT 1 FROM image_metadata im
            WHERE im.image_id = i.id
              AND (im.key LIKE ? OR im.value LIKE ?)
        )
        OR EXISTS (
            SELECT 1 FROM generation_runs gr
            WHERE gr.id = i.generation_run_id
              AND (
                gr.prompt LIKE ?
                OR gr.negative_prompt LIKE ?
                OR gr.provider LIKE ?
                OR gr.model LIKE ?
                OR gr.raw_metadata_json LIKE ?
              )
        )
    )";

    let pattern = format!("%{}%", term.trim());
    let params = std::iter::repeat(SqlValue::Text(pattern))
        .take(10)
        .collect();

    match op {
        RuleOp::Contains | RuleOp::Eq => Ok((sql.to_string(), params)),
        RuleOp::NotContains | RuleOp::Neq => Ok((format!("NOT {}", sql), params)),
        _ => Err(format!("Unsupported operator {:?} for search_text", op)),
    }
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

    #[test]
    fn test_simple_rating_filter_to_sql() {
        let filter = FilterNode::Rule {
            field: Field::Rating,
            op: RuleOp::Gte,
            value: FilterValue::Number(4.0),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();
        assert!(sql.contains("s.star_rating"));
        assert!(sql.contains(">="));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_and_group_to_sql() {
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
        let (sql, params) = filter.to_sql_clause().unwrap();
        assert!(sql.contains("AND"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_not_to_sql() {
        let filter = FilterNode::Not {
            child: Box::new(FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("midjourney".to_string()),
            }),
        };
        let (sql, _) = filter.to_sql_clause().unwrap();
        assert!(sql.contains("NOT"));
    }

    #[test]
    fn test_last_n_days_to_sql() {
        let filter = FilterNode::Rule {
            field: Field::ImportedAt,
            op: RuleOp::LastNDays,
            value: FilterValue::Number(7.0),
        };
        let (sql, _) = filter.to_sql_clause().unwrap();
        assert!(sql.contains("datetime"));
    }

    #[test]
    fn test_orientation_filter() {
        let filter = FilterNode::Rule {
            field: Field::Orientation,
            op: RuleOp::Eq,
            value: FilterValue::String("landscape".to_string()),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();
        assert!(sql.contains("i.orientation"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn test_text_search_filter_matches_file_and_metadata_text() {
        let filter: FilterNode = serde_json::from_str(
            r#"{"type":"rule","field":"search_text","op":"contains","value":"astra"}"#,
        )
        .unwrap();
        let (sql, params) = filter.to_sql_clause().unwrap();

        assert!(sql.contains("f.path"), "sql: {}", sql);
        assert!(sql.contains("image_metadata"), "sql: {}", sql);
        assert!(sql.contains("generation_runs"), "sql: {}", sql);
        assert_eq!(params.len(), 10);
        assert!(params.iter().all(|p| match p {
            SqlValue::Text(text) => text == "%astra%",
            _ => false,
        }));
    }

    #[test]
    fn test_in_operator() {
        let filter = FilterNode::Rule {
            field: Field::Format,
            op: RuleOp::In,
            value: FilterValue::StringArray(vec!["png".to_string(), "webp".to_string()]),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();
        assert!(sql.contains("IN"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_clip_field_returns_error() {
        let filter = FilterNode::Rule {
            field: Field::ClipSimilarTo,
            op: RuleOp::Eq,
            value: FilterValue::String("test".to_string()),
        };
        assert!(filter.to_sql_clause().is_err());
    }
}
