use super::tags::normalize_tag_name;
use chrono::DateTime;
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
    Tag,
    Folder,
    ImportedAt,
    OriginalDate,
    FileSize,
    FocusScore,
    BlurScore,
    ExposureScore,
    ClippedShadowPct,
    ClippedHighlightPct,
    MeanLuma,
    Contrast,
    DominantHex,
    DominantHueBucket,
    MeanSaturation,
    Colorfulness,
    SimilarityGroupId,
    ClipSimilarTo,
    ClipTextMatch,
    #[serde(rename = "catalog_field")]
    CatalogField {
        key: String,
    },
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
            Field::Tag => "tag",
            Field::Folder => "f.path",
            Field::ImportedAt => "i.imported_at",
            Field::OriginalDate => "i.original_date",
            Field::FileSize => "i.file_size",
            Field::FocusScore => "qm.focus_score",
            Field::BlurScore => "qm.blur_score",
            Field::ExposureScore => "qm.exposure_score",
            Field::ClippedShadowPct => "qm.clipped_shadow_pct",
            Field::ClippedHighlightPct => "qm.clipped_highlight_pct",
            Field::MeanLuma => "qm.mean_luma",
            Field::Contrast => "qm.contrast",
            Field::DominantHex => "cm.dominant_hex",
            Field::DominantHueBucket => "cm.dominant_hue_bucket",
            Field::MeanSaturation => "cm.mean_saturation",
            Field::Colorfulness => "cm.colorfulness",
            Field::SimilarityGroupId => "sgi.group_id",
            Field::ClipSimilarTo | Field::ClipTextMatch | Field::CatalogField { .. } => {
                "unsupported"
            }
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
                if matches!(field, Field::CatalogField { .. }) {
                    if let Field::CatalogField { key } = field {
                        return catalog_field_clause(key, op, value);
                    }
                }

                if matches!(field, Field::SearchText) {
                    return text_search_clause(op, value);
                }
                if matches!(field, Field::Tag) {
                    return tag_clause(op, value);
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
                        // An empty allow-list matches nothing; `IN ()` is invalid SQL.
                        if vals.is_empty() {
                            return Ok(("1=0".to_string(), vec![]));
                        }
                        let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
                        let params: Vec<SqlValue> =
                            vals.iter().map(|v| SqlValue::Text(v.clone())).collect();
                        Ok((format!("{} IN ({})", col, placeholders.join(",")), params))
                    }
                    (RuleOp::NotIn, FilterValue::StringArray(vals)) => {
                        // An empty deny-list excludes nothing; `NOT IN ()` is invalid SQL.
                        if vals.is_empty() {
                            return Ok(("1=1".to_string(), vec![]));
                        }
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
                    (RuleOp::LastNDays, FilterValue::Number(days)) => {
                        // `days` is interpolated into SQL, so it must be a finite,
                        // non-negative whole number. Reject NaN/inf/negative/fractional
                        // rather than silently casting to a wrong or future window.
                        // Finite, non-negative, whole, and within a sane upper
                        // bound (100 years) so astronomically large values can't
                        // produce a degenerate datetime modifier.
                        const MAX_DAYS: f64 = 36_500.0;
                        if !days.is_finite()
                            || *days < 0.0
                            || days.fract() != 0.0
                            || *days > MAX_DAYS
                        {
                            return Err(format!(
                                "last_n_days requires a whole number of days in 0..={}, got {}",
                                MAX_DAYS as i64, days
                            ));
                        }
                        Ok((
                            format!("{} >= datetime('now', '-{} days')", col, *days as i64),
                            vec![],
                        ))
                    }
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

fn catalog_field_clause(
    stable_key: &str,
    op: &RuleOp,
    value: &FilterValue,
) -> std::result::Result<(String, Vec<SqlValue>), String> {
    let key = stable_key.trim();
    if key.is_empty() {
        return Err("catalog_field requires a non-empty stable key".to_string());
    }

    let base_where = "cv.subject_type = 'image'
             AND cv.subject_id = i.id
             AND cv.status = 'approved'
             AND cfd.stable_key = ?";
    let key_param = SqlValue::Text(key.to_string());

    let with_subject = |predicate: &str| {
        format!(
            "EXISTS (
                SELECT 1
                FROM catalog_field_values cv
                JOIN catalog_field_defs cfd ON cv.field_def_id = cfd.id
                WHERE {base_where} AND {predicate}
            )",
            base_where = base_where,
            predicate = predicate
        )
    };

    match (op, value) {
        (RuleOp::Eq, FilterValue::String(v)) => Ok((
            with_subject("cv.display_value = ?"),
            vec![key_param, SqlValue::Text(v.clone())],
        )),
        (RuleOp::Eq, FilterValue::Number(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS REAL) = ?"),
            vec![key_param, SqlValue::Real(*v)],
        )),
        (RuleOp::Eq, FilterValue::Bool(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS INTEGER) = ?"),
            vec![key_param, SqlValue::Integer(*v as i64)],
        )),
        (RuleOp::Neq, FilterValue::String(v)) => Ok((
            with_subject("cv.display_value != ?"),
            vec![key_param, SqlValue::Text(v.clone())],
        )),
        (RuleOp::Neq, FilterValue::Number(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS REAL) != ?"),
            vec![key_param, SqlValue::Real(*v)],
        )),
        (RuleOp::Neq, FilterValue::Bool(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS INTEGER) != ?"),
            vec![key_param, SqlValue::Integer(*v as i64)],
        )),
        (RuleOp::Gt, FilterValue::Number(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS REAL) > ?"),
            vec![key_param, SqlValue::Real(*v)],
        )),
        (RuleOp::Gte, FilterValue::Number(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS REAL) >= ?"),
            vec![key_param, SqlValue::Real(*v)],
        )),
        (RuleOp::Lt, FilterValue::Number(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS REAL) < ?"),
            vec![key_param, SqlValue::Real(*v)],
        )),
        (RuleOp::Lte, FilterValue::Number(v)) => Ok((
            with_subject("CAST(json_extract(cv.value_json, '$.value') AS REAL) <= ?"),
            vec![key_param, SqlValue::Real(*v)],
        )),
        (RuleOp::Contains, FilterValue::String(v)) => Ok((
            with_subject("cv.display_value LIKE ?"),
            vec![key_param, SqlValue::Text(format!("%{}%", v))],
        )),
        (RuleOp::NotContains, FilterValue::String(v)) => Ok((
            with_subject("cv.display_value NOT LIKE ?"),
            vec![key_param, SqlValue::Text(format!("%{}%", v))],
        )),
        (RuleOp::In, FilterValue::StringArray(vals)) => {
            if vals.is_empty() {
                return Ok(("1=0".to_string(), vec![]));
            }
            let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
            let mut params = Vec::with_capacity(vals.len() + 1);
            params.push(key_param);
            params.extend(vals.iter().map(|v| SqlValue::Text(v.clone())));
            let predicate = format!("cv.display_value IN ({})", placeholders.join(","));
            Ok((with_subject(&predicate), params))
        }
        (RuleOp::NotIn, FilterValue::StringArray(vals)) => {
            if vals.is_empty() {
                return Ok(("1=1".to_string(), vec![]));
            }
            let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
            let mut params = Vec::with_capacity(vals.len() + 1);
            params.push(key_param);
            params.extend(vals.iter().map(|v| SqlValue::Text(v.clone())));
            let predicate = format!("cv.display_value NOT IN ({})", placeholders.join(","));
            Ok((with_subject(&predicate), params))
        }
        (RuleOp::Between, FilterValue::Range { from, to }) => {
            let from_value = from.trim().to_string();
            let to_value = to.trim().to_string();
            if from_value.is_empty() || to_value.is_empty() {
                return Err("between requires non-empty from and to values".to_string());
            }
            let is_date_range = DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", from_value))
                .is_ok()
                && DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", to_value)).is_ok();

            if is_date_range {
                Ok((
                    with_subject(
                        "date(json_extract(cv.value_json, '$.value')) BETWEEN date(?) AND date(?)",
                    ),
                    vec![
                        key_param,
                        SqlValue::Text(from_value),
                        SqlValue::Text(to_value),
                    ],
                ))
            } else {
                Ok((
                    with_subject(
                        "CAST(json_extract(cv.value_json, '$.value') AS REAL) BETWEEN ? AND ?",
                    ),
                    vec![
                        key_param,
                        SqlValue::Text(from_value),
                        SqlValue::Text(to_value),
                    ],
                ))
            }
        }
        (RuleOp::IsEmpty, _) => Ok((
            "NOT EXISTS (
                SELECT 1
                FROM catalog_field_values cv
                JOIN catalog_field_defs cfd ON cv.field_def_id = cfd.id
                WHERE cv.subject_type = 'image'
                  AND cv.subject_id = i.id
                  AND cv.status = 'approved'
                  AND cfd.stable_key = ?
             )"
            .to_string(),
            vec![key_param],
        )),
        (RuleOp::IsNotEmpty, _) => Ok((
            "EXISTS (
                SELECT 1
                FROM catalog_field_values cv
                JOIN catalog_field_defs cfd ON cv.field_def_id = cfd.id
                WHERE cv.subject_type = 'image'
                  AND cv.subject_id = i.id
                  AND cv.status = 'approved'
                  AND cfd.stable_key = ?
             )"
            .to_string(),
            vec![key_param],
        )),
        _ => Err(format!(
            "Unsupported operator {:?} for catalog field {:?}",
            op, stable_key
        )),
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
            SELECT 1 FROM image_tags it
            JOIN tags t ON t.id = it.tag_id
            WHERE it.image_id = i.id
              AND (
                t.name LIKE ?
                OR t.normalized_name LIKE ?
                OR it.source LIKE ?
              )
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
        OR EXISTS (
            SELECT 1 FROM media_assets ma
            WHERE ma.primary_image_id = i.id
              AND ma.title LIKE ?
        )
        OR EXISTS (
            SELECT 1 FROM media_assets ma
            JOIN pdf_pages pp ON pp.media_asset_id = ma.id
            WHERE ma.primary_image_id = i.id
              AND pp.extracted_text LIKE ?
        )
    )";

    let pattern = format!("%{}%", term.trim());
    let params = std::iter::repeat(SqlValue::Text(pattern))
        .take(15)
        .collect();

    match op {
        RuleOp::Contains | RuleOp::Eq => Ok((sql.to_string(), params)),
        RuleOp::NotContains | RuleOp::Neq => Ok((format!("NOT {}", sql), params)),
        _ => Err(format!("Unsupported operator {:?} for search_text", op)),
    }
}

fn tag_clause(
    op: &RuleOp,
    value: &FilterValue,
) -> std::result::Result<(String, Vec<SqlValue>), String> {
    let exists_sql = "EXISTS (
        SELECT 1 FROM image_tags it
        JOIN tags t ON t.id = it.tag_id
        WHERE it.image_id = i.id
    )";

    match (op, value) {
        (RuleOp::IsEmpty, _) => Ok((format!("NOT {}", exists_sql), vec![])),
        (RuleOp::IsNotEmpty, _) => Ok((exists_sql.to_string(), vec![])),
        (RuleOp::Eq, FilterValue::String(v)) => {
            let normalized = normalize_tag_name(v).unwrap_or_default();
            Ok((
                "EXISTS (
                    SELECT 1 FROM image_tags it
                    JOIN tags t ON t.id = it.tag_id
                    WHERE it.image_id = i.id AND t.normalized_name = ?
                )"
                .to_string(),
                vec![SqlValue::Text(normalized)],
            ))
        }
        (RuleOp::Neq, FilterValue::String(v)) => {
            let normalized = normalize_tag_name(v).unwrap_or_default();
            Ok((
                "NOT EXISTS (
                    SELECT 1 FROM image_tags it
                    JOIN tags t ON t.id = it.tag_id
                    WHERE it.image_id = i.id AND t.normalized_name = ?
                )"
                .to_string(),
                vec![SqlValue::Text(normalized)],
            ))
        }
        (RuleOp::Contains, FilterValue::String(v)) => {
            let normalized = normalize_tag_name(v).unwrap_or_else(|| v.trim().to_lowercase());
            Ok((
                "EXISTS (
                    SELECT 1 FROM image_tags it
                    JOIN tags t ON t.id = it.tag_id
                    WHERE it.image_id = i.id
                      AND (t.name LIKE ? OR t.normalized_name LIKE ? OR it.source LIKE ?)
                )"
                .to_string(),
                vec![
                    SqlValue::Text(format!("%{}%", v.trim())),
                    SqlValue::Text(format!("%{}%", normalized)),
                    SqlValue::Text(format!("%{}%", v.trim())),
                ],
            ))
        }
        (RuleOp::NotContains, FilterValue::String(v)) => {
            let normalized = normalize_tag_name(v).unwrap_or_else(|| v.trim().to_lowercase());
            Ok((
                "NOT EXISTS (
                    SELECT 1 FROM image_tags it
                    JOIN tags t ON t.id = it.tag_id
                    WHERE it.image_id = i.id
                      AND (t.name LIKE ? OR t.normalized_name LIKE ? OR it.source LIKE ?)
                )"
                .to_string(),
                vec![
                    SqlValue::Text(format!("%{}%", v.trim())),
                    SqlValue::Text(format!("%{}%", normalized)),
                    SqlValue::Text(format!("%{}%", v.trim())),
                ],
            ))
        }
        (RuleOp::In, FilterValue::StringArray(vals)) => {
            let normalized: Vec<String> =
                vals.iter().filter_map(|v| normalize_tag_name(v)).collect();
            if normalized.is_empty() {
                return Ok(("0=1".to_string(), vec![]));
            }
            let placeholders: Vec<&str> = normalized.iter().map(|_| "?").collect();
            Ok((
                format!(
                    "EXISTS (
                        SELECT 1 FROM image_tags it
                        JOIN tags t ON t.id = it.tag_id
                        WHERE it.image_id = i.id AND t.normalized_name IN ({})
                    )",
                    placeholders.join(",")
                ),
                normalized.into_iter().map(SqlValue::Text).collect(),
            ))
        }
        (RuleOp::NotIn, FilterValue::StringArray(vals)) => {
            let normalized: Vec<String> =
                vals.iter().filter_map(|v| normalize_tag_name(v)).collect();
            if normalized.is_empty() {
                return Ok(("1=1".to_string(), vec![]));
            }
            let placeholders: Vec<&str> = normalized.iter().map(|_| "?").collect();
            Ok((
                format!(
                    "NOT EXISTS (
                        SELECT 1 FROM image_tags it
                        JOIN tags t ON t.id = it.tag_id
                        WHERE it.image_id = i.id AND t.normalized_name IN ({})
                    )",
                    placeholders.join(",")
                ),
                normalized.into_iter().map(SqlValue::Text).collect(),
            ))
        }
        _ => Err(format!("Unsupported operator {:?} for tag", op)),
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
    fn in_with_empty_array_matches_nothing() {
        let filter = FilterNode::Rule {
            field: Field::SourceLabel,
            op: RuleOp::In,
            value: FilterValue::StringArray(vec![]),
        };
        // Must not emit invalid `IN ()`; an empty allow-list matches nothing.
        let (sql, params) = filter.to_sql_clause().unwrap();
        assert!(!sql.contains("IN ()"), "must not emit invalid IN (): {sql}");
        assert!(params.is_empty());
        assert!(sql.contains("1=0") || sql.contains("0=1"));
    }

    #[test]
    fn not_in_with_empty_array_matches_everything() {
        let filter = FilterNode::Rule {
            field: Field::SourceLabel,
            op: RuleOp::NotIn,
            value: FilterValue::StringArray(vec![]),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();
        assert!(
            !sql.contains("NOT IN ()"),
            "must not emit invalid NOT IN (): {sql}"
        );
        assert!(params.is_empty());
        assert!(sql.contains("1=1"));
    }

    #[test]
    fn last_n_days_rejects_non_integer_and_negative_values() {
        for bad in [-1.0, 3.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e9] {
            let filter = FilterNode::Rule {
                field: Field::ImportedAt,
                op: RuleOp::LastNDays,
                value: FilterValue::Number(bad),
            };
            assert!(
                filter.to_sql_clause().is_err(),
                "last_n_days must reject {bad}"
            );
        }
        // A valid window still compiles.
        let ok = FilterNode::Rule {
            field: Field::ImportedAt,
            op: RuleOp::LastNDays,
            value: FilterValue::Number(7.0),
        };
        assert!(ok.to_sql_clause().is_ok());
    }

    #[test]
    fn clip_similarity_fields_are_rejected_not_emitted_as_columns() {
        for field in [Field::ClipSimilarTo, Field::ClipTextMatch] {
            let filter = FilterNode::Rule {
                field,
                op: RuleOp::Gte,
                value: FilterValue::Number(0.5),
            };
            // Must error rather than emit the literal "unsupported" column.
            assert!(
                filter.to_sql_clause().is_err(),
                "CLIP similarity is not SQL-expressible and must error"
            );
        }
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
        assert!(sql.contains("image_tags"), "sql: {}", sql);
        assert!(sql.contains("generation_runs"), "sql: {}", sql);
        assert!(sql.contains("media_assets"), "sql: {}", sql);
        assert!(sql.contains("pdf_pages"), "sql: {}", sql);
        assert_eq!(params.len(), 15);
        assert!(params.iter().all(|p| match p {
            SqlValue::Text(text) => text == "%astra%",
            _ => false,
        }));
    }

    #[test]
    fn test_tag_filter_to_sql() {
        let filter: FilterNode = serde_json::from_str(
            r#"{"type":"rule","field":"tag","op":"eq","value":"Golden Hour"}"#,
        )
        .unwrap();
        let (sql, params) = filter.to_sql_clause().unwrap();

        assert!(sql.contains("image_tags"), "sql: {}", sql);
        assert!(sql.contains("normalized_name"), "sql: {}", sql);
        assert_eq!(params, vec![SqlValue::Text("golden-hour".to_string())]);
    }

    #[test]
    fn test_catalog_field_eq_filter() {
        let filter = FilterNode::Rule {
            field: Field::CatalogField {
                key: "inventory.height".to_string(),
            },
            op: RuleOp::Eq,
            value: FilterValue::String("110".to_string()),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();

        assert!(sql.contains("cv.display_value = ?"));
        assert!(sql.contains("cv.status = 'approved'"));
        assert!(sql.contains("cfd.stable_key = ?"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], SqlValue::Text("inventory.height".to_string()));
        assert_eq!(params[1], SqlValue::Text("110".to_string()));
    }

    #[test]
    fn test_catalog_field_gt_filter_uses_numeric_cast() {
        let filter = FilterNode::Rule {
            field: Field::CatalogField {
                key: "inventory.depth".to_string(),
            },
            op: RuleOp::Gt,
            value: FilterValue::Number(42.0),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();

        assert!(sql.contains("CAST(json_extract(cv.value_json, '$.value') AS REAL) > ?"));
        assert_eq!(params.len(), 2);
        assert_eq!(
            params,
            vec![
                SqlValue::Text("inventory.depth".to_string()),
                SqlValue::Real(42.0)
            ]
        );
    }

    #[test]
    fn test_catalog_field_in_filter_empty_set_matches_nothing() {
        let filter = FilterNode::Rule {
            field: Field::CatalogField {
                key: "inventory.materials".to_string(),
            },
            op: RuleOp::In,
            value: FilterValue::StringArray(vec![]),
        };
        let (sql, params) = filter.to_sql_clause().unwrap();

        assert!(sql.contains("1=0"));
        assert!(params.is_empty());
    }

    #[test]
    fn test_catalog_field_between_range_uses_date_or_numeric_cast() {
        let date_filter = FilterNode::Rule {
            field: Field::CatalogField {
                key: "creation.date".to_string(),
            },
            op: RuleOp::Between,
            value: FilterValue::Range {
                from: "2026-01-01".to_string(),
                to: "2026-01-31".to_string(),
            },
        };
        let (date_sql, params) = date_filter.to_sql_clause().unwrap();
        assert!(date_sql
            .contains("date(json_extract(cv.value_json, '$.value')) BETWEEN date(?) AND date(?)"));
        assert_eq!(params.len(), 3);
        assert_eq!(params[1], SqlValue::Text("2026-01-01".to_string()));
        assert_eq!(params[2], SqlValue::Text("2026-01-31".to_string()));

        let numeric_filter = FilterNode::Rule {
            field: Field::CatalogField {
                key: "size.width".to_string(),
            },
            op: RuleOp::Between,
            value: FilterValue::Range {
                from: "10".to_string(),
                to: "20".to_string(),
            },
        };
        let (numeric_sql, _numeric_params) = numeric_filter.to_sql_clause().unwrap();
        assert!(numeric_sql
            .contains("CAST(json_extract(cv.value_json, '$.value') AS REAL) BETWEEN ? AND ?"));
    }

    #[test]
    fn test_catalog_field_with_empty_key_rejected() {
        let filter = FilterNode::Rule {
            field: Field::CatalogField {
                key: "   ".to_string(),
            },
            op: RuleOp::Eq,
            value: FilterValue::String("x".to_string()),
        };
        assert!(filter.to_sql_clause().is_err());
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
