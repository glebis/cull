use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmpMetadata {
    pub rating: Option<u8>,
    pub color_label: Option<String>,
    pub keywords: Vec<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub copyright: Option<String>,
    pub prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub seed: Option<String>,
}

pub fn serialize_xmp(meta: &XmpMetadata) -> String {
    let mut out = String::new();
    out.push_str(r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>"#);
    out.push('\n');
    out.push_str(r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">"#);
    out.push('\n');
    out.push_str(r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">"#);
    out.push('\n');
    out.push_str(r#"<rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/" xmlns:Iptc4xmpExt="http://iptc.org/std/Iptc4xmpExt/2008-02-29/" xmlns:cull="https://cull.app/ns/exchange/1.0/">"#);
    out.push('\n');

    if let Some(rating) = meta.rating {
        out.push_str(&format!("<xmp:Rating>{}</xmp:Rating>\n", rating.min(5)));
    }
    if let Some(label) = &meta.color_label {
        out.push_str(&format!("<xmp:Label>{}</xmp:Label>\n", esc(label)));
    }
    if let Some(title) = &meta.title {
        lang_alt(&mut out, "dc:title", title);
    }
    if let Some(description) = &meta.description {
        lang_alt(&mut out, "dc:description", description);
    }
    if let Some(creator) = &meta.creator {
        out.push_str("<dc:creator><rdf:Seq>");
        out.push_str(&format!("<rdf:li>{}</rdf:li>", esc(creator)));
        out.push_str("</rdf:Seq></dc:creator>\n");
    }
    if !meta.keywords.is_empty() {
        out.push_str("<dc:subject><rdf:Bag>");
        for keyword in &meta.keywords {
            out.push_str(&format!("<rdf:li>{}</rdf:li>", esc(keyword)));
        }
        out.push_str("</rdf:Bag></dc:subject>\n");
    }
    if let Some(copyright) = &meta.copyright {
        out.push_str(&format!(
            "<dc:rights><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:rights>\n",
            esc(copyright)
        ));
    }
    if let Some(prompt) = &meta.prompt {
        out.push_str(&format!("<Iptc4xmpExt:DigitalSourceType>trainedAlgorithmicMedia</Iptc4xmpExt:DigitalSourceType>\n<cull:Prompt>{}</cull:Prompt>\n", esc(prompt)));
    }
    if let Some(provider) = &meta.provider {
        out.push_str(&format!(
            "<cull:Provider>{}</cull:Provider>\n",
            esc(provider)
        ));
    }
    if let Some(model) = &meta.model {
        out.push_str(&format!("<cull:Model>{}</cull:Model>\n", esc(model)));
    }
    if let Some(seed) = &meta.seed {
        out.push_str(&format!("<cull:Seed>{}</cull:Seed>\n", esc(seed)));
    }

    out.push_str("</rdf:Description>\n</rdf:RDF>\n</x:xmpmeta>\n");
    out.push_str(r#"<?xpacket end="w"?>"#);
    out.push('\n');
    out
}

pub fn parse_xmp(input: &str) -> XmpMetadata {
    let mut meta = XmpMetadata::default();
    meta.rating = tag(input, "xmp:Rating").and_then(|s| s.parse::<u8>().ok());
    meta.color_label = tag(input, "xmp:Label");
    meta.title = lang_alt_value(input, "dc:title").or_else(|| tag(input, "dc:title"));
    meta.description =
        lang_alt_value(input, "dc:description").or_else(|| tag(input, "dc:description"));
    meta.creator = seq_first(input, "dc:creator");
    meta.keywords = bag_values(input, "dc:subject");
    meta.copyright = lang_alt_value(input, "dc:rights");
    meta.prompt = tag(input, "cull:Prompt");
    meta.provider = tag(input, "cull:Provider");
    meta.model = tag(input, "cull:Model");
    meta.seed = tag(input, "cull:Seed");
    meta
}

fn lang_alt(out: &mut String, tag: &str, value: &str) {
    out.push_str(&format!(
        "<{tag}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{tag}>\n",
        esc(value)
    ));
}

fn tag(input: &str, name: &str) -> Option<String> {
    let pattern = format!(
        r"(?s)<{}[^>]*>(.*?)</{}>",
        regex::escape(name),
        regex::escape(name)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(input)
        .and_then(|cap| cap.get(1))
        .map(|m| unesc(m.as_str().trim()))
        .filter(|s| !s.is_empty())
}

fn lang_alt_value(input: &str, name: &str) -> Option<String> {
    let inner = tag(input, name)?;
    Regex::new(r#"(?s)<rdf:li[^>]*>(.*?)</rdf:li>"#)
        .ok()?
        .captures(&inner)
        .and_then(|cap| cap.get(1))
        .map(|m| unesc(m.as_str().trim()))
        .filter(|s| !s.is_empty())
}

fn seq_first(input: &str, name: &str) -> Option<String> {
    let inner = tag(input, name)?;
    Regex::new(r#"(?s)<rdf:li[^>]*>(.*?)</rdf:li>"#)
        .ok()?
        .captures(&inner)
        .and_then(|cap| cap.get(1))
        .map(|m| unesc(m.as_str().trim()))
        .filter(|s| !s.is_empty())
}

fn bag_values(input: &str, name: &str) -> Vec<String> {
    let Some(inner) = tag(input, name) else {
        return vec![];
    };
    let Ok(re) = Regex::new(r#"(?s)<rdf:li[^>]*>(.*?)</rdf:li>"#) else {
        return vec![];
    };
    re.captures_iter(&inner)
        .filter_map(|cap| cap.get(1))
        .map(|m| unesc(m.as_str().trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

fn esc(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unesc(value: &str) -> String {
    value
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_serialization_is_deterministic_and_round_trips() {
        let meta = XmpMetadata {
            rating: Some(4),
            color_label: Some("green".to_string()),
            keywords: vec!["portrait".to_string(), "client".to_string()],
            title: Some("Frame 01".to_string()),
            description: Some("A <generated> image".to_string()),
            creator: Some("Cull".to_string()),
            copyright: Some("Copyright 2026".to_string()),
            prompt: Some("cinematic light".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-image-1".to_string()),
            seed: Some("123".to_string()),
        };

        let a = serialize_xmp(&meta);
        let b = serialize_xmp(&meta);
        assert_eq!(a, b);
        assert_eq!(parse_xmp(&a), meta);
    }

    #[test]
    fn malformed_xmp_returns_partial_metadata() {
        let parsed = parse_xmp("<xmp:Rating>5</xmp:Rating><cull:Prompt>hello");
        assert_eq!(parsed.rating, Some(5));
        assert_eq!(parsed.prompt, None);
    }
}
