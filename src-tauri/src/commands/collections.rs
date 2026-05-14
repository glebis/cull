use crate::db_core::models::{ImageWithFile, NewSessionEvent};
use crate::services::curation as svc;
use crate::services::{Pagination, ServiceContext};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let id = svc::create_collection(&ctx, &name).map_err(|e| e.to_string())?;
    let _ = state.db.log_session_event(&NewSessionEvent {
        session_id: None,
        event_type: "collection_created".to_string(),
        actor_type: "user".to_string(),
        actor_id: None,
        subject_type: Some("collection".to_string()),
        subject_id: Some(id.clone()),
        payload_json: serde_json::json!({ "name": name }).to_string(),
    });
    Ok(id)
}

#[tauri::command]
pub async fn list_collections(
    state: State<'_, AppState>,
) -> Result<Vec<(String, String, u32)>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_collections(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_collection(
    state: State<'_, AppState>,
    collection_id: String,
    image_ids: Vec<String>,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::add_to_collection(&ctx, &collection_id, &refs).map_err(|e| e.to_string())?;
    let _ = state.db.log_session_event(&NewSessionEvent {
        session_id: None,
        event_type: "collection_items_added".to_string(),
        actor_type: "user".to_string(),
        actor_id: None,
        subject_type: Some("collection".to_string()),
        subject_id: Some(collection_id),
        payload_json: serde_json::json!({ "image_count": image_ids.len() }).to_string(),
    });
    Ok(())
}

#[tauri::command]
pub async fn list_collection_images(
    state: State<'_, AppState>,
    collection_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    if let Some(limit) = limit {
        svc::list_collection_images_page(
            &ctx,
            &collection_id,
            Pagination::clamped(offset.unwrap_or(0), limit),
        )
        .map_err(|e| e.to_string())
    } else {
        svc::list_collection_images(&ctx, &collection_id).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn remove_from_collection(
    state: State<'_, AppState>,
    collection_id: String,
    image_ids: Vec<String>,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::remove_from_collection(&ctx, &collection_id, &refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_collection(&ctx, &collection_id).map_err(|e| e.to_string())?;
    let _ = state.db.log_session_event(&NewSessionEvent {
        session_id: None,
        event_type: "collection_deleted".to_string(),
        actor_type: "user".to_string(),
        actor_id: None,
        subject_type: Some("collection".to_string()),
        subject_id: Some(collection_id),
        payload_json: "{}".to_string(),
    });
    Ok(())
}
