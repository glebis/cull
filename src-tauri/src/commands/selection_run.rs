// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by AI tools. See AUTHORSHIP.md.

//! Native Selection Mode commands. Mutations emit `selection-run:updated`
//! with `{ selection_id }` so every surface can refresh canonical state.

use crate::db_core::models::{
    SelectionPage, SelectionRun, SelectionSourceScope, SelectionState, ShortlistMutationResult,
};
use crate::services::selection_run as svc;
use crate::services::selection_run::{SelectionPageFilters, ShortlistActor, ShortlistDirection};
use crate::AppState;
use tauri::{AppHandle, Emitter, State};

const SELECTION_RUN_UPDATED: &str = "selection-run:updated";

#[derive(serde::Serialize)]
pub struct SelectionSourcePreview {
    pub count: u32,
}

fn emit_run_updated(app: &AppHandle, selection_id: &str) {
    let _ = app.emit(
        SELECTION_RUN_UPDATED,
        serde_json::json!({ "selection_id": selection_id }),
    );
}

#[tauri::command]
pub async fn preview_selection_source(
    state: State<'_, AppState>,
    source_scope: SelectionSourceScope,
) -> Result<SelectionSourcePreview, String> {
    svc::preview_selection_source(&state.db, &source_scope)
        .map(|count| SelectionSourcePreview { count })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_selection_run(
    state: State<'_, AppState>,
    app: AppHandle,
    name: String,
    source_scope: SelectionSourceScope,
    target_count: Option<u32>,
) -> Result<SelectionState, String> {
    let created = svc::create_selection_run(&state.db, &name, &source_scope, target_count)
        .map_err(|e| e.to_string())?;
    emit_run_updated(&app, &created.run.id);
    Ok(created)
}

#[tauri::command]
pub async fn list_selection_runs(
    state: State<'_, AppState>,
    status: Option<String>,
) -> Result<Vec<SelectionRun>, String> {
    svc::list_selection_runs(&state.db, status.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_selection_run(
    state: State<'_, AppState>,
    selection_id: String,
) -> Result<SelectionState, String> {
    svc::get_selection_state(&state.db, &selection_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_selection_source(
    state: State<'_, AppState>,
    selection_id: String,
    offset: u32,
    limit: u32,
    query: Option<String>,
    min_size: Option<u32>,
    include_rejected: Option<bool>,
) -> Result<SelectionPage, String> {
    svc::list_source_page(
        &state.db,
        &selection_id,
        offset,
        limit,
        SelectionPageFilters {
            query,
            min_size,
            include_rejected: include_rejected.unwrap_or(false),
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_selection_shortlist(
    state: State<'_, AppState>,
    selection_id: String,
    offset: u32,
    limit: u32,
    query: Option<String>,
    min_size: Option<u32>,
    include_rejected: Option<bool>,
) -> Result<SelectionPage, String> {
    svc::list_shortlist_page(
        &state.db,
        &selection_id,
        offset,
        limit,
        SelectionPageFilters {
            query,
            min_size,
            include_rejected: include_rejected.unwrap_or(false),
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_shortlist(
    state: State<'_, AppState>,
    app: AppHandle,
    selection_id: String,
    image_ids: Vec<String>,
) -> Result<SelectionState, String> {
    mutate_shortlist(
        &state,
        &app,
        &selection_id,
        &image_ids,
        ShortlistDirection::Add,
    )
}

#[tauri::command]
pub async fn remove_from_shortlist(
    state: State<'_, AppState>,
    app: AppHandle,
    selection_id: String,
    image_ids: Vec<String>,
) -> Result<SelectionState, String> {
    mutate_shortlist(
        &state,
        &app,
        &selection_id,
        &image_ids,
        ShortlistDirection::Remove,
    )
}

fn mutate_shortlist(
    state: &State<'_, AppState>,
    app: &AppHandle,
    selection_id: &str,
    image_ids: &[String],
    direction: ShortlistDirection,
) -> Result<SelectionState, String> {
    let ShortlistMutationResult {
        state: new_state, ..
    } = svc::apply_shortlist_change(
        &state.db,
        &state.action_manager,
        selection_id,
        image_ids,
        direction,
        ShortlistActor::User,
    )
    .map_err(|e| e.to_string())?;
    emit_run_updated(app, selection_id);
    Ok(new_state)
}

#[tauri::command]
pub async fn finish_selection_run(
    state: State<'_, AppState>,
    app: AppHandle,
    selection_id: String,
) -> Result<SelectionState, String> {
    let finished =
        svc::finish_selection_run(&state.db, &selection_id).map_err(|e| e.to_string())?;
    emit_run_updated(&app, &selection_id);
    Ok(finished)
}

#[tauri::command]
pub async fn reopen_selection_run(
    state: State<'_, AppState>,
    app: AppHandle,
    selection_id: String,
) -> Result<SelectionState, String> {
    let reopened =
        svc::reopen_selection_run(&state.db, &selection_id).map_err(|e| e.to_string())?;
    emit_run_updated(&app, &selection_id);
    Ok(reopened)
}

#[tauri::command]
pub async fn archive_selection_run(
    state: State<'_, AppState>,
    app: AppHandle,
    selection_id: String,
) -> Result<SelectionState, String> {
    let archived =
        svc::archive_selection_run(&state.db, &selection_id).map_err(|e| e.to_string())?;
    emit_run_updated(&app, &selection_id);
    Ok(archived)
}

#[tauri::command]
pub async fn restore_selection_run(
    state: State<'_, AppState>,
    app: AppHandle,
    selection_id: String,
) -> Result<SelectionState, String> {
    let restored =
        svc::restore_selection_run(&state.db, &selection_id).map_err(|e| e.to_string())?;
    emit_run_updated(&app, &selection_id);
    Ok(restored)
}
