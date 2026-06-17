use serde::Deserialize;
use serde_json::Value;

use super::HeadlessContext;

#[derive(Debug, Deserialize)]
struct ListCatalogFieldsParams {
    pub subject_scope: Option<String>,
    pub include_deprecated: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateCatalogFieldDefParams {
    pub stable_key: String,
    pub label: String,
    pub description: Option<String>,
    pub subject_scope: String,
    pub value_type: String,
    pub cardinality: String,
    pub unit_kind: Option<String>,
    pub validation_json: Option<String>,
    pub sensitivity: String,
    pub derived_source: Option<String>,
    pub crosswalk_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCatalogPresetParams {
    pub name: String,
    pub description: Option<String>,
    pub preset_kind: String,
    pub field_def_ids: Vec<String>,
    pub layout_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCatalogPresetParams {
    pub preset_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub field_def_ids: Option<Vec<String>>,
    pub layout_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogPresetIdParam {
    pub preset_id: String,
}

#[derive(Debug, Deserialize)]
struct CatalogFieldDefIdParam {
    pub field_def_id: String,
}

#[derive(Debug, Deserialize)]
struct CatalogImageIdParam {
    pub primary_image_id: String,
}

#[derive(Debug, Deserialize)]
struct CatalogRecordParams {
    pub subject_type: String,
    pub subject_id: String,
}

#[derive(Debug, Deserialize)]
struct JobIdParam {
    pub job_id: String,
}

#[derive(Debug, Deserialize)]
struct AttachCatalogImageInput {
    pub image_id: String,
    pub role: String,
    pub ordinal: i64,
    pub edition_label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AttachImagesToCatalogWorkParams {
    pub work_id: String,
    pub images: Vec<AttachCatalogImageInput>,
}

#[derive(Debug, Deserialize)]
struct ListCatalogValuesParams {
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub status: Option<String>,
    pub source_type: Option<String>,
    pub field_def_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetCatalogDraftValueParams {
    pub subject_type: String,
    pub subject_id: String,
    pub field_def_id: String,
    pub value_json: String,
    pub display_value: String,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CatalogDraftValueInput {
    pub subject_type: String,
    pub subject_id: String,
    pub field_def_id: String,
    pub value_json: String,
    pub display_value: String,
    pub confidence: Option<f64>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetCatalogDraftValuesParams {
    pub values: Vec<CatalogDraftValueInput>,
}

pub fn list_catalog_presets(ctx: &HeadlessContext) -> Result<Value, String> {
    let presets = ctx.db.list_catalog_presets().map_err(|e| e.to_string())?;
    serde_json::to_value(presets).map_err(|e| e.to_string())
}

pub fn get_catalog_preset(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: CatalogPresetIdParam = serde_json::from_value(params)
        .map_err(|e| format!("Invalid get_catalog_preset params: {}", e))?;
    let result = ctx
        .db
        .get_catalog_preset(&parsed.preset_id)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

pub fn list_catalog_fields(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: ListCatalogFieldsParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid list_catalog_fields params: {}", e))?;
    let fields = ctx
        .db
        .list_catalog_fields(
            parsed.subject_scope.as_deref(),
            parsed.include_deprecated.unwrap_or(false),
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_value(fields).map_err(|e| e.to_string())
}

pub fn create_catalog_field_def(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: CreateCatalogFieldDefParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid create_catalog_field_def params: {}", e))?;
    let id = ctx
        .db
        .create_catalog_field_def(
            &parsed.stable_key,
            &parsed.label,
            parsed.description.as_deref(),
            &parsed.subject_scope,
            &parsed.value_type,
            &parsed.cardinality,
            parsed.unit_kind.as_deref(),
            parsed.validation_json.as_deref(),
            &parsed.sensitivity,
            parsed.derived_source.as_deref(),
            parsed.crosswalk_json.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "id": id }))
}

pub fn deprecate_catalog_field_def(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: CatalogFieldDefIdParam = serde_json::from_value(params)
        .map_err(|e| format!("Invalid deprecate_catalog_field_def params: {}", e))?;
    ctx.db
        .deprecate_catalog_field_def(&parsed.field_def_id)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "status": "ok" }))
}

pub fn create_catalog_preset(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: CreateCatalogPresetParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid create_catalog_preset params: {}", e))?;
    let id = ctx
        .db
        .create_catalog_preset(
            &parsed.name,
            parsed.description.as_deref(),
            &parsed.preset_kind,
            &parsed.field_def_ids,
            parsed.layout_json.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "id": id }))
}

pub fn update_catalog_preset(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: UpdateCatalogPresetParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid update_catalog_preset params: {}", e))?;
    let field_def_ids = parsed.field_def_ids.as_deref();
    ctx.db
        .update_catalog_preset(
            &parsed.preset_id,
            parsed.name.as_deref(),
            parsed.description.as_deref(),
            field_def_ids,
            parsed.layout_json.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "status": "ok", "id": parsed.preset_id }))
}

pub fn create_catalog_work(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: CatalogImageIdParam = serde_json::from_value(params)
        .map_err(|e| format!("Invalid create_catalog_work params: {}", e))?;
    let id = ctx
        .db
        .create_catalog_work(&parsed.primary_image_id)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "id": id }))
}

pub fn attach_images_to_catalog_work(
    ctx: &HeadlessContext,
    params: Value,
) -> Result<Value, String> {
    let parsed: AttachImagesToCatalogWorkParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid attach_images_to_catalog_work params: {}", e))?;
    let prepared: Vec<(String, String, i64, Option<String>)> = parsed
        .images
        .into_iter()
        .map(|image| {
            (
                image.image_id,
                image.role,
                image.ordinal,
                image.edition_label,
            )
        })
        .collect();
    let attached = ctx
        .db
        .attach_images_to_catalog_work(&parsed.work_id, &prepared)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "attached": attached }))
}

pub fn list_catalog_values(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: ListCatalogValuesParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid list_catalog_values params: {}", e))?;
    let values = ctx
        .db
        .list_catalog_values(
            parsed.subject_type.as_deref(),
            parsed.subject_id.as_deref(),
            parsed.status.as_deref(),
            parsed.source_type.as_deref(),
            parsed.field_def_id.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_value(values).map_err(|e| e.to_string())
}

pub fn list_catalog_drafts(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    #[derive(Debug, Deserialize)]
    struct ListCatalogDraftsParams {
        pub subject_type: Option<String>,
        pub subject_id: Option<String>,
        pub source_type: Option<String>,
    }

    let parsed: ListCatalogDraftsParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid list_catalog_drafts params: {}", e))?;
    let values = ctx
        .db
        .list_catalog_drafts(
            parsed.subject_type.as_deref(),
            parsed.subject_id.as_deref(),
            parsed.source_type.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_value(values).map_err(|e| e.to_string())
}

pub fn get_catalog_record(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: CatalogRecordParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid get_catalog_record params: {}", e))?;
    let result = ctx
        .db
        .get_catalog_record(&parsed.subject_type, &parsed.subject_id)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

pub fn set_catalog_draft_value(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: SetCatalogDraftValueParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid set_catalog_draft_value params: {}", e))?;
    let source_type = parsed.source_type.unwrap_or_else(|| "user".to_string());
    let id = ctx
        .db
        .upsert_catalog_draft_value(
            &parsed.subject_type,
            &parsed.subject_id,
            &parsed.field_def_id,
            &parsed.value_json,
            &parsed.display_value,
            &source_type,
            parsed.source_id.as_deref(),
            parsed.confidence,
            "cli",
            None,
            "draft",
        )
        .map_err(|e| e.to_string())?;
    serde_json::to_value(id).map_err(|e| e.to_string())
}

pub fn set_catalog_draft_values(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: SetCatalogDraftValuesParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid set_catalog_draft_values params: {}", e))?;
    #[expect(
        clippy::type_complexity,
        reason = "headless catalog batch payload mirrors the database batch tuple"
    )]
    let payload: Vec<(
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<f64>,
        Option<String>,
    )> = parsed
        .values
        .into_iter()
        .map(|value| {
            (
                value.subject_type,
                value.subject_id,
                value.field_def_id,
                value.value_json,
                value.display_value,
                value.source_type,
                value.confidence,
                value.source_id,
            )
        })
        .collect();
    let ids = ctx
        .db
        .set_catalog_draft_values(&payload, "cli", None)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(ids).map_err(|e| e.to_string())
}

pub fn suggest_catalog_values(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: SetCatalogDraftValuesParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid suggest_catalog_values params: {}", e))?;
    let mut drafted_ids = Vec::new();
    for value in parsed.values {
        let source_type = value.source_type.unwrap_or_else(|| "agent".to_string());
        let value_id = ctx
            .db
            .upsert_catalog_draft_value(
                &value.subject_type,
                &value.subject_id,
                &value.field_def_id,
                &value.value_json,
                &value.display_value,
                &source_type,
                value.source_id.as_deref(),
                value.confidence,
                "cli",
                None,
                "draft",
            )
            .map_err(|e| e.to_string())?;
        drafted_ids.push(value_id);
    }
    Ok(serde_json::json!({
        "status": "completed",
        "drafted_count": drafted_ids.len(),
        "written_count": drafted_ids.len(),
        "ids": drafted_ids,
    }))
}

pub fn get_catalog_suggestion_job(_ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: JobIdParam = serde_json::from_value(params)
        .map_err(|e| format!("Invalid get_catalog_suggestion_job params: {}", e))?;
    Ok(serde_json::json!({
        "job_id": parsed.job_id,
        "status": "not_supported_via_cli"
    }))
}

pub fn approve_catalog_values(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: VecValueIds = serde_json::from_value(params)
        .map_err(|e| format!("Invalid approve_catalog_values params: {}", e))?;
    let count = ctx
        .db
        .approve_catalog_values(&parsed.value_ids, Some("cli"))
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "updated": count }))
}

pub fn reject_catalog_values(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: VecValueIds = serde_json::from_value(params)
        .map_err(|e| format!("Invalid reject_catalog_values params: {}", e))?;
    let count = ctx
        .db
        .reject_catalog_values(&parsed.value_ids, Some("cli"))
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "updated": count }))
}

#[derive(Debug, serde::Deserialize)]
struct VecValueIds {
    pub value_ids: Vec<String>,
}
