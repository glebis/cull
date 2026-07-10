use crate::commands::log_library_event;
use crate::db_core::models::ClientFeedback;
use crate::services::undo::Action;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn set_rating(
    state: State<'_, AppState>,
    image_id: String,
    rating: u8,
    session_id: Option<String>,
) -> Result<(), String> {
    state.action_manager.execute(
        &state.db,
        Action::SetRating {
            image_id: image_id.clone(),
            rating,
        },
    )?;
    let mut payload = serde_json::json!({ "image_id": image_id.clone(), "rating": rating });
    if let (Some(object), Some(session_id)) = (payload.as_object_mut(), session_id) {
        object.insert(
            "session_id".to_string(),
            serde_json::Value::String(session_id),
        );
    }
    log_library_event(&state, "rating_set", Some("image"), Some(image_id), payload);
    Ok(())
}

#[tauri::command]
pub async fn set_decision(
    state: State<'_, AppState>,
    image_id: String,
    decision: String,
    session_id: Option<String>,
) -> Result<(), String> {
    state.action_manager.execute(
        &state.db,
        Action::SetDecision {
            image_id: image_id.clone(),
            decision: decision.clone(),
        },
    )?;
    let mut payload = serde_json::json!({ "image_id": image_id.clone(), "decision": decision });
    if let (Some(object), Some(session_id)) = (payload.as_object_mut(), session_id) {
        object.insert(
            "session_id".to_string(),
            serde_json::Value::String(session_id),
        );
    }
    log_library_event(
        &state,
        "decision_set",
        Some("image"),
        Some(image_id),
        payload,
    );
    Ok(())
}

/// Client feedback (favorite + comment) is stored separately from curator
/// selections so the two never overwrite each other.
#[tauri::command]
pub async fn set_client_feedback(
    state: State<'_, AppState>,
    image_id: String,
    favorite: bool,
    comment: Option<String>,
) -> Result<(), String> {
    state
        .db
        .set_client_feedback(&image_id, favorite, comment.as_deref())
        .map_err(|e| e.to_string())?;
    log_library_event(
        &state,
        "client_feedback_set",
        Some("image"),
        Some(image_id.clone()),
        serde_json::json!({
            "image_id": image_id,
            "favorite": favorite,
            "has_comment": comment.as_deref().is_some_and(|value| !value.trim().is_empty()),
        }),
    );
    Ok(())
}

#[tauri::command]
pub async fn get_client_feedback(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<ClientFeedback>, String> {
    state
        .db
        .get_client_feedback(&image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_client_feedback(
    state: State<'_, AppState>,
) -> Result<Vec<ClientFeedback>, String> {
    state.db.list_client_feedback().map_err(|e| e.to_string())
}
