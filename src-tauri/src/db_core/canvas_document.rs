use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

pub const CANVAS_DOCUMENT_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDocument {
    pub version: u8,
    #[serde(default)]
    pub viewport: CanvasViewport,
    #[serde(default)]
    pub items: Vec<CanvasItem>,
    #[serde(default)]
    pub groups: Vec<CanvasGroup>,
    #[serde(default)]
    pub connectors: Vec<CanvasConnector>,
    #[serde(default)]
    pub annotations: Vec<CanvasAnnotation>,
    #[serde(default)]
    pub export: CanvasExportIntent,
}

impl CanvasDocument {
    pub fn empty() -> Self {
        Self {
            version: CANVAS_DOCUMENT_VERSION,
            viewport: CanvasViewport::default(),
            items: Vec::new(),
            groups: Vec::new(),
            connectors: Vec::new(),
            annotations: Vec::new(),
            export: CanvasExportIntent::default(),
        }
    }

    pub fn from_layout_json(layout_json: &str) -> Result<Self, CanvasDocumentError> {
        if layout_json.trim().is_empty() {
            return Ok(Self::empty());
        }

        let value: serde_json::Value = serde_json::from_str(layout_json)?;
        if value.as_object().is_some_and(|object| object.is_empty()) {
            return Ok(Self::empty());
        }

        let document: Self = serde_json::from_value(value)?;
        document.validate()?;
        Ok(document)
    }

    #[allow(dead_code)]
    pub fn to_layout_json(&self) -> Result<String, CanvasDocumentError> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }

    pub fn validate(&self) -> Result<(), CanvasDocumentError> {
        let errors = validate_canvas_document(self);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(CanvasDocumentError::new(errors))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasViewport {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasItem {
    pub id: String,
    pub image_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z: i32,
    pub hidden: bool,
    pub label: Option<String>,
    pub group_id: Option<String>,
    #[serde(default)]
    pub transform: CanvasItemTransform,
    #[serde(default)]
    pub source: CanvasImageReference,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasImageReference {
    pub content_hash: Option<String>,
    pub last_known_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasItemTransform {
    pub crop: Option<CanvasCrop>,
    pub rotation_degrees: f64,
    pub fit: CanvasItemFit,
}

impl Default for CanvasItemTransform {
    fn default() -> Self {
        Self {
            crop: None,
            rotation_degrees: 0.0,
            fit: CanvasItemFit::Contain,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasCrop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CanvasItemFit {
    Contain,
    Cover,
    Stretch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasGroup {
    pub id: String,
    pub name: String,
    pub item_ids: Vec<String>,
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z: i32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasConnector {
    pub id: String,
    pub from_item_id: String,
    pub to_item_id: String,
    pub relationship: CanvasConnectorKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasConnectorKind {
    Lineage,
    Variant,
    Reference,
    Sequence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasAnnotation {
    pub id: String,
    pub target: CanvasAnnotationTarget,
    pub body: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub created_at: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CanvasAnnotationTarget {
    #[serde(rename = "canvas")]
    Canvas,
    #[serde(rename = "item")]
    Item {
        #[serde(rename = "itemId")]
        item_id: String,
    },
    #[serde(rename = "group")]
    Group {
        #[serde(rename = "groupId")]
        group_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CanvasExportIntent {
    pub default_preset_id: Option<String>,
    pub background: CanvasExportBackground,
    pub bounds: CanvasExportBounds,
}

impl Default for CanvasExportIntent {
    fn default() -> Self {
        Self {
            default_preset_id: None,
            background: CanvasExportBackground::Transparent,
            bounds: CanvasExportBounds::Content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasExportBackground {
    Transparent,
    Canvas,
    White,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CanvasExportBounds {
    Content,
    Viewport,
    Selection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasDocumentError {
    errors: Vec<String>,
}

impl CanvasDocumentError {
    fn new(errors: Vec<String>) -> Self {
        Self { errors }
    }
}

impl fmt::Display for CanvasDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.errors.join("; "))
    }
}

impl std::error::Error for CanvasDocumentError {}

impl From<serde_json::Error> for CanvasDocumentError {
    fn from(error: serde_json::Error) -> Self {
        Self {
            errors: vec![error.to_string()],
        }
    }
}

fn validate_canvas_document(document: &CanvasDocument) -> Vec<String> {
    let mut errors = Vec::new();

    if document.version != CANVAS_DOCUMENT_VERSION {
        errors.push(format!(
            "unsupported canvas document version {}",
            document.version
        ));
    }

    validate_finite("viewport panX", document.viewport.pan_x, &mut errors);
    validate_finite("viewport panY", document.viewport.pan_y, &mut errors);
    if !document.viewport.zoom.is_finite() || document.viewport.zoom <= 0.0 {
        errors.push("viewport zoom must be greater than 0".to_string());
    }

    let mut item_ids = HashSet::new();
    for item in &document.items {
        if item.id.trim().is_empty() {
            errors.push("item id must not be empty".to_string());
        } else if !item_ids.insert(item.id.as_str()) {
            errors.push(format!("duplicate item id {}", item.id));
        }

        if item.image_id.trim().is_empty() {
            errors.push(format!("item {} imageId must not be empty", item.id));
        }
        validate_finite(&format!("item {} x", item.id), item.x, &mut errors);
        validate_finite(&format!("item {} y", item.id), item.y, &mut errors);
        validate_positive(&format!("item {} width", item.id), item.width, &mut errors);
        validate_positive(
            &format!("item {} height", item.id),
            item.height,
            &mut errors,
        );
        validate_finite(
            &format!("item {} rotationDegrees", item.id),
            item.transform.rotation_degrees,
            &mut errors,
        );
        if let Some(crop) = &item.transform.crop {
            validate_finite(&format!("item {} crop x", item.id), crop.x, &mut errors);
            validate_finite(&format!("item {} crop y", item.id), crop.y, &mut errors);
            validate_positive(
                &format!("item {} crop width", item.id),
                crop.width,
                &mut errors,
            );
            validate_positive(
                &format!("item {} crop height", item.id),
                crop.height,
                &mut errors,
            );
        }
    }

    let mut group_ids = HashSet::new();
    for group in &document.groups {
        if group.id.trim().is_empty() {
            errors.push("group id must not be empty".to_string());
        } else if !group_ids.insert(group.id.as_str()) {
            errors.push(format!("duplicate group id {}", group.id));
        }

        validate_finite(&format!("group {} x", group.id), group.x, &mut errors);
        validate_finite(&format!("group {} y", group.id), group.y, &mut errors);
        validate_positive(
            &format!("group {} width", group.id),
            group.width,
            &mut errors,
        );
        validate_positive(
            &format!("group {} height", group.id),
            group.height,
            &mut errors,
        );

        for item_id in &group.item_ids {
            if !item_ids.contains(item_id.as_str()) {
                errors.push(format!(
                    "group {} references missing item {}",
                    group.id, item_id
                ));
            }
        }
    }

    for item in &document.items {
        if let Some(group_id) = &item.group_id {
            if !group_ids.contains(group_id.as_str()) {
                errors.push(format!(
                    "item {} references missing group {}",
                    item.id, group_id
                ));
            }
        }
    }

    let mut connector_ids = HashSet::new();
    for connector in &document.connectors {
        if connector.id.trim().is_empty() {
            errors.push("connector id must not be empty".to_string());
        } else if !connector_ids.insert(connector.id.as_str()) {
            errors.push(format!("duplicate connector id {}", connector.id));
        }

        if !item_ids.contains(connector.from_item_id.as_str()) {
            errors.push(format!(
                "connector {} references missing item {}",
                connector.id, connector.from_item_id
            ));
        }
        if !item_ids.contains(connector.to_item_id.as_str()) {
            errors.push(format!(
                "connector {} references missing item {}",
                connector.id, connector.to_item_id
            ));
        }
    }

    let mut annotation_ids = HashSet::new();
    for annotation in &document.annotations {
        if annotation.id.trim().is_empty() {
            errors.push("annotation id must not be empty".to_string());
        } else if !annotation_ids.insert(annotation.id.as_str()) {
            errors.push(format!("duplicate annotation id {}", annotation.id));
        }

        if annotation.body.trim().is_empty() {
            errors.push(format!(
                "annotation {} body must not be empty",
                annotation.id
            ));
        }
        if let Some(x) = annotation.x {
            validate_finite(&format!("annotation {} x", annotation.id), x, &mut errors);
        }
        if let Some(y) = annotation.y {
            validate_finite(&format!("annotation {} y", annotation.id), y, &mut errors);
        }
        match &annotation.target {
            CanvasAnnotationTarget::Canvas => {}
            CanvasAnnotationTarget::Item { item_id } => {
                if !item_ids.contains(item_id.as_str()) {
                    errors.push(format!(
                        "annotation {} references missing item {}",
                        annotation.id, item_id
                    ));
                }
            }
            CanvasAnnotationTarget::Group { group_id } => {
                if !group_ids.contains(group_id.as_str()) {
                    errors.push(format!(
                        "annotation {} references missing group {}",
                        annotation.id, group_id
                    ));
                }
            }
        }
    }

    errors
}

fn validate_finite(label: &str, value: f64, errors: &mut Vec<String>) {
    if !value.is_finite() {
        errors.push(format!("{label} must be finite"));
    }
}

fn validate_positive(label: &str, value: f64, errors: &mut Vec<String>) {
    if !value.is_finite() || value <= 0.0 {
        errors.push(format!("{label} must be greater than 0"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_document() -> CanvasDocument {
        CanvasDocument {
            version: CANVAS_DOCUMENT_VERSION,
            viewport: CanvasViewport::default(),
            items: vec![CanvasItem {
                id: "item-1".to_string(),
                image_id: "img-1".to_string(),
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 200.0,
                z: 0,
                hidden: false,
                label: Some("Hero".to_string()),
                group_id: Some("group-1".to_string()),
                transform: CanvasItemTransform::default(),
                source: CanvasImageReference {
                    content_hash: Some("hash-1".to_string()),
                    last_known_path: Some("/photos/hero.png".to_string()),
                },
            }],
            groups: vec![CanvasGroup {
                id: "group-1".to_string(),
                name: "Selects".to_string(),
                item_ids: vec!["item-1".to_string()],
                label: Some("Selects".to_string()),
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 300.0,
                z: 0,
                collapsed: false,
            }],
            connectors: vec![CanvasConnector {
                id: "connector-1".to_string(),
                from_item_id: "item-1".to_string(),
                to_item_id: "item-1".to_string(),
                relationship: CanvasConnectorKind::Lineage,
                label: Some("variant".to_string()),
            }],
            annotations: vec![CanvasAnnotation {
                id: "note-1".to_string(),
                target: CanvasAnnotationTarget::Item {
                    item_id: "item-1".to_string(),
                },
                body: "Needs export review".to_string(),
                x: Some(12.0),
                y: Some(18.0),
                created_at: None,
                author: None,
            }],
            export: CanvasExportIntent::default(),
        }
    }

    #[test]
    fn parses_legacy_empty_layout_as_empty_v1_document() {
        let doc = CanvasDocument::from_layout_json("{}").unwrap();

        assert_eq!(doc.version, CANVAS_DOCUMENT_VERSION);
        assert!(doc.items.is_empty());
        assert!(doc.groups.is_empty());
        assert!(doc.connectors.is_empty());
        assert!(doc.annotations.is_empty());
    }

    #[test]
    fn round_trips_valid_document_json() {
        let doc = valid_document();

        let json = doc.to_layout_json().unwrap();
        let parsed = CanvasDocument::from_layout_json(&json).unwrap();

        assert_eq!(parsed, doc);
    }

    #[test]
    fn serializes_frontend_facing_field_names() {
        let value = serde_json::to_value(valid_document()).unwrap();

        assert!(value["viewport"].get("panX").is_some());
        assert_eq!(value["items"][0]["imageId"], "img-1");
        assert_eq!(value["items"][0]["source"]["contentHash"], "hash-1");
        assert_eq!(value["annotations"][0]["target"]["itemId"], "item-1");
        assert!(value["annotations"][0]["target"].get("item_id").is_none());
    }

    #[test]
    fn rejects_invalid_version() {
        let mut value = serde_json::to_value(valid_document()).unwrap();
        value["version"] = serde_json::json!(2);

        let err = CanvasDocument::from_layout_json(&value.to_string()).unwrap_err();

        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn rejects_duplicate_item_ids() {
        let mut doc = valid_document();
        let mut duplicate = doc.items[0].clone();
        duplicate.image_id = "img-2".to_string();
        doc.items.push(duplicate);

        let err = doc.validate().unwrap_err();

        assert!(err.to_string().contains("duplicate item id"));
    }

    #[test]
    fn rejects_invalid_item_dimensions() {
        let mut doc = valid_document();
        doc.items[0].width = 0.0;

        let err = doc.validate().unwrap_err();

        assert!(err.to_string().contains("width"));
    }

    #[test]
    fn rejects_bad_group_connector_and_annotation_references() {
        let mut doc = valid_document();
        doc.groups[0].item_ids = vec!["missing-item".to_string()];
        doc.connectors[0].to_item_id = "missing-item".to_string();
        doc.annotations[0].target = CanvasAnnotationTarget::Item {
            item_id: "missing-item".to_string(),
        };

        let err = doc.validate().unwrap_err();
        let text = err.to_string();

        assert!(text.contains("group group-1 references missing item missing-item"));
        assert!(text.contains("connector connector-1 references missing item missing-item"));
        assert!(text.contains("annotation note-1 references missing item missing-item"));
    }
}
