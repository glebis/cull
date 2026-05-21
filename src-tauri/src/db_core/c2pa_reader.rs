use std::path::Path;

#[derive(Debug, Clone)]
pub struct C2paInfo {
    pub software_agents: Vec<String>,
    pub has_chatgpt_layer: bool,
}

impl C2paInfo {
    pub fn openai_source_label(&self) -> Option<&'static str> {
        let agents_lower: Vec<String> = self
            .software_agents
            .iter()
            .map(|a| a.to_lowercase())
            .collect();
        let is_openai = agents_lower
            .iter()
            .any(|a| a.contains("openai") || a.contains("gpt") || a.contains("dall"));

        if !is_openai {
            return None;
        }

        if agents_lower
            .iter()
            .any(|a| a.contains("gpt-image") || a.contains("gpt_image") || a.contains("gpt image"))
        {
            Some("gpt_image_2")
        } else if agents_lower
            .iter()
            .any(|a| a.contains("dall-e") || a.contains("dall·e") || a.contains("dalle"))
        {
            Some("dalle_3")
        } else {
            Some("openai")
        }
    }
}

pub fn read_c2pa_info(path: &Path) -> Option<C2paInfo> {
    let data = std::fs::read(path).ok()?;
    let ext = path.extension()?.to_str()?.to_lowercase();

    let jumbf_bytes = match ext.as_str() {
        "jpg" | "jpeg" => extract_jumbf_from_jpeg(&data),
        "png" => extract_jumbf_from_png(&data),
        "webp" => extract_jumbf_from_riff(&data),
        _ => None,
    }?;

    let agents = extract_software_agents(&jumbf_bytes);
    if agents.is_empty() {
        return None;
    }

    let has_chatgpt_layer = agents.iter().any(|a| a.to_lowercase().contains("chatgpt"));

    Some(C2paInfo {
        software_agents: agents,
        has_chatgpt_layer,
    })
}

fn extract_jumbf_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let mut jumbf = Vec::new();
    let mut pos = 2; // skip SOI

    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }

    while pos + 4 < data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = data[pos + 1];
        if marker == 0xD9 {
            break; // EOI
        }
        if marker == 0x00 || (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }

        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if pos + 2 + seg_len > data.len() {
            break;
        }

        // APP11 = 0xEB, used for JUMBF C2PA
        if marker == 0xEB && seg_len > 18 {
            let payload_start = pos + 4;
            let payload_end = pos + 2 + seg_len;
            // JUMBF boxes inside APP11 start after the "JP" header bytes
            if payload_end <= data.len() {
                jumbf.extend_from_slice(&data[payload_start..payload_end]);
            }
        }

        pos += 2 + seg_len;
    }

    if jumbf.is_empty() {
        None
    } else {
        Some(jumbf)
    }
}

fn extract_jumbf_from_png(data: &[u8]) -> Option<Vec<u8>> {
    let mut jumbf = Vec::new();

    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }

    let mut pos = 8;
    while pos + 12 <= data.len() {
        let chunk_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let chunk_data_start = pos + 8;
        let chunk_data_end = chunk_data_start + chunk_len;

        if chunk_data_end + 4 > data.len() {
            break;
        }

        // caBX chunk contains C2PA JUMBF data
        if chunk_type == b"caBX" {
            jumbf.extend_from_slice(&data[chunk_data_start..chunk_data_end]);
        }

        pos = chunk_data_end + 4; // skip CRC
    }

    if jumbf.is_empty() {
        None
    } else {
        Some(jumbf)
    }
}

fn extract_jumbf_from_riff(data: &[u8]) -> Option<Vec<u8>> {
    // WebP uses RIFF container; C2PA stored in "C2PA" chunk
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let chunk_data_start = pos + 8;
        let chunk_data_end = chunk_data_start + chunk_size;

        if chunk_data_end > data.len() {
            break;
        }

        if chunk_id == b"C2PA" {
            return Some(data[chunk_data_start..chunk_data_end].to_vec());
        }

        pos = chunk_data_end + (chunk_size % 2); // RIFF pads to even
    }

    None
}

fn extract_software_agents(jumbf: &[u8]) -> Vec<String> {
    let mut agents = Vec::new();

    // The softwareAgent in OpenAI C2PA manifests is a CBOR map:
    //   softwareAgent → {name: "gpt-image", version: "pre-2.0"}
    // We extract all readable strings after "softwareAgent" and reconstruct
    // the agent identity from name + version fields.
    let needle = b"softwareAgent";

    for i in 0..jumbf.len().saturating_sub(needle.len()) {
        if &jumbf[i..i + needle.len()] == needle {
            let window_end = (i + needle.len() + 128).min(jumbf.len());
            let window = &jumbf[i + needle.len()..window_end];
            let strings = extract_cbor_strings(window);

            // Look for name/version pattern: strings come in pairs after CBOR keys
            // e.g. ["name", "gpt-image", "version", "pre-2.0"]
            let mut name = None;
            let mut version = None;
            for (j, s) in strings.iter().enumerate() {
                if s == "name" {
                    name = strings.get(j + 1).cloned();
                } else if s == "version" {
                    version = strings.get(j + 1).cloned();
                }
            }

            let agent = match (name, version) {
                (Some(n), Some(v)) => format!("{} {}", n, v),
                (Some(n), None) => n,
                _ => {
                    // Fallback: if it's a plain string (not a map), take first readable string
                    strings
                        .into_iter()
                        .find(|s| s.len() >= 3)
                        .unwrap_or_default()
                }
            };

            if !agent.is_empty() && !agents.contains(&agent) {
                agents.push(agent);
            }
        }
    }

    // Also scan for "OpenAI" certificate issuer as a secondary signal
    let openai_needle = b"OpenAI";
    let has_openai_cert = jumbf
        .windows(openai_needle.len())
        .any(|w| w == openai_needle);
    if has_openai_cert && agents.is_empty() {
        agents.push("OpenAI (certificate only)".to_string());
    }

    agents
}

fn extract_cbor_strings(data: &[u8]) -> Vec<String> {
    // Extract CBOR text strings from a byte window.
    // CBOR text strings: major type 3 (0x60-0x77 for lengths 0-23, 0x78 for 1-byte length)
    let mut strings = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let byte = data[pos];
        let major = byte >> 5;
        let additional = byte & 0x1f;

        if major == 3 {
            // Text string
            let (str_len, header_size) = match additional {
                0..=23 => (additional as usize, 1),
                24 if pos + 1 < data.len() => (data[pos + 1] as usize, 2),
                25 if pos + 3 < data.len() => (
                    u16::from_be_bytes([data[pos + 1], data[pos + 2]]) as usize,
                    3,
                ),
                _ => {
                    pos += 1;
                    continue;
                }
            };

            let str_start = pos + header_size;
            let str_end = str_start + str_len;
            if str_end <= data.len() {
                if let Ok(s) = std::str::from_utf8(&data[str_start..str_end]) {
                    if !s.is_empty() {
                        strings.push(s.to_string());
                    }
                }
                pos = str_end;
                continue;
            }
        }

        // For map/array headers, just skip the header byte
        if major == 5 || major == 4 {
            pos += 1;
            continue;
        }

        pos += 1;

        // Stop after we've collected enough to identify the agent
        if strings.len() >= 8 {
            break;
        }
    }

    strings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_source_label_gpt_image_2() {
        let info = C2paInfo {
            software_agents: vec!["OpenAI GPT-image-2".to_string()],
            has_chatgpt_layer: false,
        };
        assert_eq!(info.openai_source_label(), Some("gpt_image_2"));
    }

    #[test]
    fn test_openai_source_label_dalle() {
        let info = C2paInfo {
            software_agents: vec!["OpenAI DALL-E 3".to_string()],
            has_chatgpt_layer: false,
        };
        assert_eq!(info.openai_source_label(), Some("dalle_3"));
    }

    #[test]
    fn test_openai_source_label_chatgpt() {
        let info = C2paInfo {
            software_agents: vec!["OpenAI GPT-image-2".to_string(), "ChatGPT".to_string()],
            has_chatgpt_layer: true,
        };
        assert_eq!(info.openai_source_label(), Some("gpt_image_2"));
        assert!(info.has_chatgpt_layer);
    }

    #[test]
    fn test_non_openai_returns_none() {
        let info = C2paInfo {
            software_agents: vec!["Adobe Photoshop".to_string()],
            has_chatgpt_layer: false,
        };
        assert_eq!(info.openai_source_label(), None);
    }

    #[test]
    fn test_invalid_jpeg() {
        assert!(extract_jumbf_from_jpeg(&[0x00, 0x00]).is_none());
    }

    #[test]
    fn test_invalid_png() {
        assert!(extract_jumbf_from_png(&[0x00; 10]).is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_real_gpt_image_2_file() {
        let path = PathBuf::from("/Users/example/cull-test-images/gpt-image-c2pa.png");
        if !path.exists() {
            crate::safe_eprintln!("Skipping: test image not found");
            return;
        }
        let info = read_c2pa_info(&path).expect("Should find C2PA data");
        crate::safe_eprintln!("Agents: {:?}", info.software_agents);
        crate::safe_eprintln!("ChatGPT layer: {}", info.has_chatgpt_layer);
        crate::safe_eprintln!("Source label: {:?}", info.openai_source_label());
        assert_eq!(info.openai_source_label(), Some("gpt_image_2"));
    }
}
