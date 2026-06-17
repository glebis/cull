use crate::db_core::models::{CatalogFieldDef, CatalogFieldValue, CatalogPreset, CatalogRecord};
use crate::services::jobs::JobSnapshot;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogWorkImageInput {
    pub image_id: String,
    pub role: String,
    pub ordinal: i64,
    pub edition_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogDraftValueInput {
    pub subject_type: String,
    pub subject_id: String,
    pub field_def_id: String,
    pub value_json: String,
    pub display_value: String,
    pub confidence: Option<f64>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestCatalogValuesResult {
    pub job_id: String,
    pub drafted_count: u32,
    pub written_count: u32,
}

#[tauri::command]
pub async fn list_catalog_presets(
    state: State<'_, AppState>,
) -> Result<Vec<CatalogPreset>, String> {
    state.db.list_catalog_presets().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_catalog_preset(
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<Option<CatalogPreset>, String> {
    state
        .db
        .get_catalog_preset(&preset_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_catalog_fields(
    state: State<'_, AppState>,
    subject_scope: Option<String>,
    include_deprecated: Option<bool>,
) -> Result<Vec<CatalogFieldDef>, String> {
    state
        .db
        .list_catalog_fields(
            subject_scope.as_deref(),
            include_deprecated.unwrap_or(false),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri IPC command parameters are the public catalog field wire contract"
)]
pub async fn create_catalog_field_def(
    state: State<'_, AppState>,
    stable_key: String,
    label: String,
    description: Option<String>,
    subject_scope: String,
    value_type: String,
    cardinality: String,
    unit_kind: Option<String>,
    validation_json: Option<String>,
    sensitivity: String,
    derived_source: Option<String>,
    crosswalk_json: Option<String>,
) -> Result<String, String> {
    state
        .db
        .create_catalog_field_def(
            &stable_key,
            &label,
            description.as_deref(),
            &subject_scope,
            &value_type,
            &cardinality,
            unit_kind.as_deref(),
            validation_json.as_deref(),
            &sensitivity,
            derived_source.as_deref(),
            crosswalk_json.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn deprecate_catalog_field_def(
    state: State<'_, AppState>,
    field_def_id: String,
) -> Result<(), String> {
    state
        .db
        .deprecate_catalog_field_def(&field_def_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_catalog_preset(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    preset_kind: String,
    field_def_ids: Vec<String>,
    layout_json: Option<String>,
) -> Result<String, String> {
    state
        .db
        .create_catalog_preset(
            &name,
            description.as_deref(),
            &preset_kind,
            &field_def_ids,
            layout_json.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_catalog_preset(
    state: State<'_, AppState>,
    preset_id: String,
    name: Option<String>,
    description: Option<String>,
    field_def_ids: Option<Vec<String>>,
    layout_json: Option<String>,
) -> Result<(), String> {
    let field_defs = field_def_ids.as_deref();
    state
        .db
        .update_catalog_preset(
            &preset_id,
            name.as_deref(),
            description.as_deref(),
            field_defs,
            layout_json.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_catalog_work(
    state: State<'_, AppState>,
    primary_image_id: String,
) -> Result<String, String> {
    state
        .db
        .create_catalog_work(&primary_image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn attach_images_to_catalog_work(
    state: State<'_, AppState>,
    work_id: String,
    images: Vec<CatalogWorkImageInput>,
) -> Result<u32, String> {
    let prepared: Vec<(String, String, i64, Option<String>)> = images
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
    state
        .db
        .attach_images_to_catalog_work(&work_id, &prepared)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_catalog_values(
    state: State<'_, AppState>,
    subject_type: Option<String>,
    subject_id: Option<String>,
    status: Option<String>,
    source_type: Option<String>,
    field_def_id: Option<String>,
) -> Result<Vec<CatalogFieldValue>, String> {
    state
        .db
        .list_catalog_values(
            subject_type.as_deref(),
            subject_id.as_deref(),
            status.as_deref(),
            source_type.as_deref(),
            field_def_id.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_catalog_drafts(
    state: State<'_, AppState>,
    subject_type: Option<String>,
    subject_id: Option<String>,
    source_type: Option<String>,
) -> Result<Vec<CatalogFieldValue>, String> {
    state
        .db
        .list_catalog_drafts(
            subject_type.as_deref(),
            subject_id.as_deref(),
            source_type.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_catalog_record(
    state: State<'_, AppState>,
    subject_type: String,
    subject_id: String,
) -> Result<Option<CatalogRecord>, String> {
    state
        .db
        .get_catalog_record(&subject_type, &subject_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "Tauri IPC command parameters mirror one catalog draft value payload"
)]
pub async fn set_catalog_draft_value(
    state: State<'_, AppState>,
    subject_type: String,
    subject_id: String,
    field_def_id: String,
    value_json: String,
    display_value: String,
    source_type: Option<String>,
    source_id: Option<String>,
    confidence: Option<f64>,
) -> Result<String, String> {
    let source_type = source_type.unwrap_or_else(|| "user".to_string());
    state
        .db
        .upsert_catalog_draft_value(
            &subject_type,
            &subject_id,
            &field_def_id,
            &value_json,
            &display_value,
            &source_type,
            source_id.as_deref(),
            confidence,
            "user",
            None,
            "draft",
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_catalog_draft_values(
    state: State<'_, AppState>,
    values: Vec<CatalogDraftValueInput>,
) -> Result<Vec<String>, String> {
    #[expect(
        clippy::type_complexity,
        reason = "database batch API accepts positional draft-value fields to avoid per-row allocation wrappers"
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
    )> = values
        .into_iter()
        .map(|value| {
            let source_type = value.source_type.unwrap_or_else(|| "user".to_string());
            (
                value.subject_type,
                value.subject_id,
                value.field_def_id,
                value.value_json,
                value.display_value,
                Some(source_type),
                value.confidence,
                value.source_id,
            )
        })
        .collect();

    state
        .db
        .set_catalog_draft_values(&payload, "user", None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn suggest_catalog_values(
    state: State<'_, AppState>,
    values: Vec<CatalogDraftValueInput>,
) -> Result<SuggestCatalogValuesResult, String> {
    let (job_id, _cancel) = state
        .jobs
        .create_job("catalog-suggest", values.len() as u32);
    let mut drafted_ids: HashMap<String, usize> = HashMap::new();
    for value in values.into_iter() {
        if state.jobs.is_cancelled(&job_id) {
            state.jobs.complete(&job_id);
            return Ok(SuggestCatalogValuesResult {
                job_id,
                drafted_count: drafted_ids.values().sum::<usize>() as u32,
                written_count: drafted_ids.values().sum::<usize>() as u32,
            });
        }

        let source_type = value.source_type.unwrap_or_else(|| "agent".to_string());
        let value_id = state.db.upsert_catalog_draft_value(
            &value.subject_type,
            &value.subject_id,
            &value.field_def_id,
            &value.value_json,
            &value.display_value,
            &source_type,
            value.source_id.as_deref(),
            value.confidence,
            "agent",
            value.source_id.as_deref(),
            "draft",
        );
        if let Ok(id) = value_id {
            drafted_ids.insert(id, 1);
        } else if let Err(err) = value_id {
            state.jobs.fail(&job_id, &err.to_string());
            return Err(err.to_string());
        }
    }

    state.jobs.complete(&job_id);
    let drafted_count = drafted_ids.len() as u32;
    Ok(SuggestCatalogValuesResult {
        job_id,
        drafted_count,
        written_count: drafted_count,
    })
}

#[tauri::command]
pub async fn get_catalog_suggestion_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<Option<JobSnapshot>, String> {
    Ok(state.jobs.get(&job_id))
}

#[tauri::command]
pub async fn approve_catalog_values(
    state: State<'_, AppState>,
    value_ids: Vec<String>,
    approved_by: Option<String>,
) -> Result<u32, String> {
    state
        .db
        .approve_catalog_values(&value_ids, approved_by.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reject_catalog_values(
    state: State<'_, AppState>,
    value_ids: Vec<String>,
    approved_by: Option<String>,
) -> Result<u32, String> {
    state
        .db
        .reject_catalog_values(&value_ids, approved_by.as_deref())
        .map_err(|e| e.to_string())
}
