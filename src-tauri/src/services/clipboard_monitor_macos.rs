use crate::services::clipboard_monitor::{
    ClipboardAccessStatus, ClipboardCapture, ClipboardImageReader,
};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardTypeHTML,
    NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardTypeTIFF, NSPasteboardTypeURL,
};
use objc2_foundation::NSData;

pub struct MacPasteboardReader {
    last_change_count: i64,
}

impl MacPasteboardReader {
    pub fn new() -> Self {
        Self {
            last_change_count: -1,
        }
    }
}

impl ClipboardImageReader for MacPasteboardReader {
    fn status(&self) -> ClipboardAccessStatus {
        ClipboardAccessStatus::Supported
    }

    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String> {
        read_macos_pasteboard_if_changed(&mut self.last_change_count)
    }
}

fn read_macos_pasteboard_if_changed(
    last_change_count: &mut i64,
) -> Result<Option<ClipboardCapture>, String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount() as i64;
    if change_count == *last_change_count {
        return Ok(None);
    }
    *last_change_count = change_count;

    let source_url = read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeURL })
        .or_else(|| read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeString }))
        .or_else(|| {
            extract_first_url(&read_string_for_type(
                &pasteboard,
                unsafe { NSPasteboardTypeHTML },
            )?)
        });

    if let Some(file_url) = read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeFileURL }) {
        if let Some(path) = file_url.strip_prefix("file://") {
            let decoded = percent_decode_file_url(path);
            let path = std::path::PathBuf::from(decoded);
            if path.exists() {
                let bytes = std::fs::read(&path)
                    .map_err(|e| format!("Failed to read clipboard file URL: {}", e))?;
                let extension = path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("png")
                    .to_string();
                let original_filename = path.file_name().map(|name| name.to_string_lossy().to_string());
                return Ok(Some(ClipboardCapture {
                    bytes,
                    extension,
                    original_filename,
                    source_url,
                    source_app: None,
                    change_count: Some(change_count),
                }));
            }
        }
    }

    if let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypePNG }) {
        return Ok(Some(ClipboardCapture {
            bytes: nsdata_to_vec(&data),
            extension: "png".to_string(),
            original_filename: None,
            source_url,
            source_app: None,
            change_count: Some(change_count),
        }));
    }

    if let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypeTIFF }) {
        return Ok(Some(ClipboardCapture {
            bytes: nsdata_to_vec(&data),
            extension: "tiff".to_string(),
            original_filename: None,
            source_url,
            source_app: None,
            change_count: Some(change_count),
        }));
    }

    Ok(None)
}

fn read_string_for_type(pasteboard: &NSPasteboard, ty: &NSPasteboardType) -> Option<String> {
    pasteboard.stringForType(ty).map(|value| value.to_string())
}

fn nsdata_to_vec(data: &NSData) -> Vec<u8> {
    let len = data.length() as usize;
    let mut bytes = vec![0u8; len];
    if len > 0 {
        let ptr =
            std::ptr::NonNull::new(bytes.as_mut_ptr().cast()).expect("vec pointer is not null");
        unsafe { data.getBytes_length(ptr, len) };
    }
    bytes
}

fn extract_first_url(value: &str) -> Option<String> {
    value
        .split(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\'')
        .find(|part| part.starts_with("http://") || part.starts_with("https://"))
        .map(|part| part.trim_end_matches(['<', '>', ')']).to_string())
}

fn percent_decode_file_url(value: &str) -> String {
    value.replace("%20", " ")
}
