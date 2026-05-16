pub fn normalize_tag_name(name: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut last_was_dash = false;

    for ch in name.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_was_dash = false;
        } else if ch.is_whitespace() || matches!(ch, '-' | '_' | '/' | '\\' | ':' | '.') {
            if !last_was_dash && !normalized.is_empty() {
                normalized.push('-');
                last_was_dash = true;
            }
        }
    }

    let normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn split_tag_list(value: &str) -> Vec<String> {
    value
        .split(|ch| matches!(ch, ',' | ';' | '\n' | '|'))
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tag_names() {
        assert_eq!(
            normalize_tag_name(" Golden Hour "),
            Some("golden-hour".to_string())
        );
        assert_eq!(
            normalize_tag_name("source:Midjourney"),
            Some("source-midjourney".to_string())
        );
        assert_eq!(normalize_tag_name("!!!"), None);
    }

    #[test]
    fn splits_common_tag_lists() {
        assert_eq!(
            split_tag_list("sunset, ocean; golden hour|portrait"),
            vec!["sunset", "ocean", "golden hour", "portrait"]
        );
    }
}
