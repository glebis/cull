use regex::Regex;
use std::sync::LazyLock;

use crate::db_core::smart_collections::*;

struct PatternRule {
    pattern: &'static LazyLock<Regex>,
    build: fn(&regex::Captures) -> Option<FilterNode>,
}

static RATING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\d)\s*\+?\s*stars?\s*(or\s*more|and\s*above|\+)?").unwrap());

static MIDJOURNEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(midjourney|mj)\b").unwrap());

static SD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(stable\s*diffusion|sd|a1111|automatic1111)\b").unwrap());

static GPT_IMAGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(gpt[\s\-_]?image[\s\-_]?2?|image[\s\-_]?gen[\s\-_]?2)\b").unwrap()
});

static CHATGPT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bchatgpt\b").unwrap());

static OPENAI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bopenai\b").unwrap());

static DALLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(dall[\-·\.]?e)\b").unwrap());

static COMFYUI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(comfyui|comfy)\b").unwrap());

static NANOBANANA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bnanobanana\b").unwrap());

static LANDSCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(landscape|horizontal|wide)\b").unwrap());

static PORTRAIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(portrait|vertical|tall)\b").unwrap());

static SQUARE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bsquare\b").unwrap());

static FORMAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(png|jpg|jpeg|webp|gif|bmp|tiff)\b").unwrap());

static RECENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(recent|today|new)\s*(imports?|images?)?").unwrap());

static THIS_WEEK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bthis\s*week\b").unwrap());

static THIS_MONTH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bthis\s*month\b").unwrap());

static PICKS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(picks?|accepted|selected)\b").unwrap());

static REJECTS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(rejects?|rejected)\b").unwrap());

static COLOR_LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(red|green|blue|yellow)\s*(label)?\b").unwrap());

static AI_GENERATED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(ai\s*generated|ai\s*images?|generated)\b").unwrap());

static PHOTOS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bphotos?\b").unwrap());

fn get_patterns() -> Vec<PatternRule> {
    vec![
        PatternRule {
            pattern: &RATING_RE,
            build: |caps| {
                let n: f64 = caps[1].parse().ok()?;
                let has_plus = caps.get(2).is_some() || caps[0].contains('+');
                Some(FilterNode::Rule {
                    field: Field::Rating,
                    op: if has_plus || n >= 4.0 {
                        RuleOp::Gte
                    } else {
                        RuleOp::Eq
                    },
                    value: FilterValue::Number(n),
                })
            },
        },
        PatternRule {
            pattern: &MIDJOURNEY_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("midjourney".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &SD_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("stable_diffusion".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &GPT_IMAGE_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("gpt_image_2".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &CHATGPT_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::In,
                    value: FilterValue::StringArray(vec![
                        "gpt_image_2".to_string(),
                        "dalle_3".to_string(),
                        "dalle".to_string(),
                    ]),
                })
            },
        },
        PatternRule {
            pattern: &OPENAI_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::In,
                    value: FilterValue::StringArray(vec![
                        "gpt_image_2".to_string(),
                        "dalle_3".to_string(),
                        "dalle".to_string(),
                        "openai".to_string(),
                    ]),
                })
            },
        },
        PatternRule {
            pattern: &DALLE_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::In,
                    value: FilterValue::StringArray(vec![
                        "dalle_3".to_string(),
                        "dalle".to_string(),
                    ]),
                })
            },
        },
        PatternRule {
            pattern: &COMFYUI_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("comfyui".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &NANOBANANA_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("nanobanana".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &LANDSCAPE_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::Orientation,
                    op: RuleOp::Eq,
                    value: FilterValue::String("landscape".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &PORTRAIT_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::Orientation,
                    op: RuleOp::Eq,
                    value: FilterValue::String("portrait".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &SQUARE_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::Orientation,
                    op: RuleOp::Eq,
                    value: FilterValue::String("square".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &FORMAT_RE,
            build: |caps| {
                Some(FilterNode::Rule {
                    field: Field::Format,
                    op: RuleOp::Eq,
                    value: FilterValue::String(caps[1].to_lowercase()),
                })
            },
        },
        PatternRule {
            pattern: &RECENT_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::ImportedAt,
                    op: RuleOp::LastNDays,
                    value: FilterValue::Number(7.0),
                })
            },
        },
        PatternRule {
            pattern: &THIS_WEEK_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::ImportedAt,
                    op: RuleOp::ThisWeek,
                    value: FilterValue::Bool(true),
                })
            },
        },
        PatternRule {
            pattern: &THIS_MONTH_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::ImportedAt,
                    op: RuleOp::ThisMonth,
                    value: FilterValue::Bool(true),
                })
            },
        },
        PatternRule {
            pattern: &PICKS_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::Decision,
                    op: RuleOp::Eq,
                    value: FilterValue::String("accept".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &REJECTS_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::Decision,
                    op: RuleOp::Eq,
                    value: FilterValue::String("reject".to_string()),
                })
            },
        },
        PatternRule {
            pattern: &COLOR_LABEL_RE,
            build: |caps| {
                Some(FilterNode::Rule {
                    field: Field::ColorLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String(caps[1].to_lowercase()),
                })
            },
        },
        PatternRule {
            pattern: &AI_GENERATED_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::IsAiGenerated,
                    op: RuleOp::Eq,
                    value: FilterValue::Bool(true),
                })
            },
        },
        PatternRule {
            pattern: &PHOTOS_RE,
            build: |_| {
                Some(FilterNode::Rule {
                    field: Field::IsAiGenerated,
                    op: RuleOp::Eq,
                    value: FilterValue::Bool(false),
                })
            },
        },
    ]
}

pub fn parse_query(input: &str) -> FilterNode {
    let input = input.trim();
    if input.is_empty() {
        return FilterNode::Group {
            op: GroupOp::And,
            children: vec![],
        };
    }

    let patterns = get_patterns();
    let mut rules: Vec<FilterNode> = vec![];
    let mut remaining = input.to_string();

    for pattern_rule in &patterns {
        if let Some(caps) = pattern_rule.pattern.captures(&remaining) {
            if let Some(node) = (pattern_rule.build)(&caps) {
                let match_info = caps.get(0).unwrap();
                let is_negated = {
                    let before = &remaining[..match_info.start()];
                    before.trim().to_lowercase().ends_with("not")
                };

                if is_negated {
                    rules.push(FilterNode::Not {
                        child: Box::new(node),
                    });
                } else {
                    rules.push(node);
                }

                let matched_text = &caps[0];
                remaining = remaining.replace(matched_text, " ");
            }
        }
    }

    if let Some(text) = remaining_search_text(&remaining) {
        rules.push(FilterNode::Rule {
            field: Field::SearchText,
            op: RuleOp::Contains,
            value: FilterValue::String(text),
        });
    }

    match rules.len() {
        0 => FilterNode::Group {
            op: GroupOp::And,
            children: vec![],
        },
        1 => rules.remove(0),
        _ => FilterNode::Group {
            op: GroupOp::And,
            children: rules,
        },
    }
}

fn remaining_search_text(input: &str) -> Option<String> {
    let terms: Vec<String> = input
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| c.is_ascii_punctuation() && c != '-' && c != '_'))
        .filter(|word| !word.is_empty())
        .filter(|word| !is_search_stopword(word))
        .map(ToString::to_string)
        .collect();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

fn is_search_stopword(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "a" | "an"
            | "and"
            | "file"
            | "files"
            | "find"
            | "for"
            | "image"
            | "images"
            | "in"
            | "not"
            | "of"
            | "or"
            | "photo"
            | "photos"
            | "picture"
            | "pictures"
            | "search"
            | "show"
            | "the"
            | "with"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_five_stars() {
        let result = parse_query("5 stars");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("star_rating"));
    }

    #[test]
    fn test_parse_midjourney() {
        let result = parse_query("midjourney");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"));
    }

    #[test]
    fn test_parse_recent() {
        let result = parse_query("recent imports");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("imported_at"));
    }

    #[test]
    fn test_parse_landscape_4_stars() {
        let result = parse_query("landscape 4 stars or more");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("orientation"));
        assert!(sql.contains("star_rating"));
    }

    #[test]
    fn test_parse_not_midjourney() {
        let result = parse_query("not midjourney");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("NOT"));
    }

    #[test]
    fn test_parse_png_images() {
        let result = parse_query("png images");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("format"));
    }

    #[test]
    fn test_parse_plain_term_falls_back_to_text_search() {
        let result = parse_query("astra");
        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["type"], "rule");
        assert_eq!(value["field"], "search_text");
        assert_eq!(value["op"], "contains");
        assert_eq!(value["value"], "astra");
    }

    #[test]
    fn test_parse_structured_query_keeps_remaining_text_search() {
        let result = parse_query("landscape astra");
        let value = serde_json::to_value(result).unwrap();
        let children = value["children"].as_array().unwrap();

        assert!(children.iter().any(|child| child["field"] == "orientation"));
        assert!(children
            .iter()
            .any(|child| child["field"] == "search_text" && child["value"] == "astra"));
    }

    #[test]
    fn test_parse_png_images_does_not_search_stopword_images() {
        let result = parse_query("png images");
        let value = serde_json::to_value(result).unwrap();
        let serialized = value.to_string();
        assert!(
            !serialized.contains("search_text"),
            "png images should stay a format filter: {}",
            serialized
        );
    }

    #[test]
    fn test_parse_picks() {
        let result = parse_query("picks");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("decision"));
    }

    #[test]
    fn test_parse_empty_returns_all() {
        let result = parse_query("");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert_eq!(sql, "1=1");
    }

    use rusqlite::types::Value as SqlValue;

    fn params_contain(params: &[SqlValue], needle: &str) -> bool {
        params.iter().any(|p| match p {
            SqlValue::Text(t) => t.contains(needle),
            _ => false,
        })
    }

    #[test]
    fn test_parse_gpt_image_2() {
        let result = parse_query("gpt image 2");
        let (sql, params) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"), "sql: {}", sql);
        assert!(
            params_contain(&params, "gpt_image_2"),
            "params: {:?}",
            params
        );
    }

    #[test]
    fn test_parse_gpt_image_hyphenated() {
        let result = parse_query("gpt-image-2");
        let (sql, params) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"), "sql: {}", sql);
        assert!(
            params_contain(&params, "gpt_image_2"),
            "params: {:?}",
            params
        );
    }

    #[test]
    fn test_parse_image_gen_2() {
        let result = parse_query("image gen 2");
        let (sql, params) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"), "sql: {}", sql);
        assert!(
            params_contain(&params, "gpt_image_2"),
            "params: {:?}",
            params
        );
    }

    #[test]
    fn test_parse_openai_umbrella() {
        let result = parse_query("openai");
        let (sql, params) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"), "sql: {}", sql);
        assert!(
            params_contain(&params, "gpt_image_2"),
            "params: {:?}",
            params
        );
        assert!(params_contain(&params, "dalle"), "params: {:?}", params);
    }

    #[test]
    fn test_parse_chatgpt() {
        let result = parse_query("chatgpt");
        let (sql, params) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"), "sql: {}", sql);
        assert!(
            params_contain(&params, "gpt_image_2"),
            "params: {:?}",
            params
        );
    }

    #[test]
    fn test_parse_dalle_only() {
        let result = parse_query("dall-e");
        let (sql, params) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"), "sql: {}", sql);
        assert!(params_contain(&params, "dalle"), "params: {:?}", params);
    }
}
