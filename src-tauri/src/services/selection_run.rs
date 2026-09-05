// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by AI tools. See AUTHORSHIP.md.

//! Selection Mode service layer: scope preview, run lifecycle, paged
//! source/shortlist reads and the grouped shortlist mutation helper.
//!
//! The [`apply_shortlist_change`] helper is the single mutation entry point.
//! It is reusable by the agent-proposal path: callers pass the explicit,
//! reviewed image IDs and the acting [`ShortlistActor`]; validation, the
//! atomic membership change, the undo record and provenance logging all live
//! here so proposals can never bypass source or lifecycle invariants.

use crate::db_core::db::Database;
use crate::db_core::models::{
    SelectionPage, SelectionRun, SelectionSourceScope, SelectionState, ShortlistMutationResult,
};
use crate::db_core::queries::selection_runs::PageFilters;
use crate::services::undo::{Action, ActionManager, ActionResult};
use crate::services::ServiceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortlistDirection {
    Add,
    Remove,
}

/// Who performed the mutation; recorded in session event provenance.
/// The `Agent` variant is constructed by the agent-proposal integration
/// (peer-owned `services/agent_proposals.rs`) via `apply_shortlist_change`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortlistActor {
    User,
    #[allow(dead_code)]
    Agent {
        id: Option<String>,
    },
}

impl ShortlistActor {
    fn actor_type(&self) -> &'static str {
        match self {
            ShortlistActor::User => "user",
            ShortlistActor::Agent { .. } => "agent",
        }
    }

    fn actor_id(&self) -> Option<String> {
        match self {
            ShortlistActor::User => None,
            ShortlistActor::Agent { id } => id.clone(),
        }
    }
}

fn service_err(error: rusqlite::Error) -> ServiceError {
    match error {
        rusqlite::Error::InvalidParameterName(message) => ServiceError::InvalidInput(message),
        other => ServiceError::Database(other),
    }
}

pub fn preview_selection_source(
    db: &Database,
    source_scope: &SelectionSourceScope,
) -> Result<u32, ServiceError> {
    let count = db
        .resolve_selection_scope_ids(source_scope)
        .map_err(service_err)?
        .len();
    u32::try_from(count)
        .map_err(|_| ServiceError::InvalidInput("Source exceeds the supported maximum".into()))
}

pub fn create_selection_run(
    db: &Database,
    name: &str,
    source_scope: &SelectionSourceScope,
    target_count: Option<u32>,
) -> Result<SelectionState, ServiceError> {
    if name.trim().is_empty() {
        return Err(ServiceError::InvalidInput(
            "Selection name must not be empty".into(),
        ));
    }
    if target_count == Some(0) {
        return Err(ServiceError::InvalidInput(
            "Target count must be at least 1".into(),
        ));
    }
    let id = db
        .create_selection_run(name.trim(), source_scope, target_count)
        .map_err(service_err)?;
    get_selection_state(db, &id)
}

pub fn list_selection_runs(
    db: &Database,
    status: Option<&str>,
) -> Result<Vec<SelectionRun>, ServiceError> {
    if let Some(status) = status {
        if !matches!(status, "active" | "finished" | "archived") {
            return Err(ServiceError::InvalidInput(format!(
                "Unknown selection run status '{status}'"
            )));
        }
    }
    db.list_selection_runs(status).map_err(service_err)
}

pub fn get_selection_state(
    db: &Database,
    selection_id: &str,
) -> Result<SelectionState, ServiceError> {
    let run = db
        .get_selection_run(selection_id)
        .map_err(service_err)?
        .ok_or_else(|| {
            ServiceError::NotFound(format!("Selection run '{selection_id}' was not found"))
        })?;
    let shortlist_ids = db
        .selection_shortlist_ids(selection_id)
        .map_err(service_err)?;
    Ok(SelectionState { run, shortlist_ids })
}

/// View filters layered within a scope (the captured scope itself never
/// changes after start).
#[derive(Debug, Clone, Default)]
pub struct SelectionPageFilters {
    pub query: Option<String>,
    pub min_size: Option<u32>,
    pub include_rejected: bool,
}

pub fn list_source_page(
    db: &Database,
    selection_id: &str,
    offset: u32,
    limit: u32,
    filters: SelectionPageFilters,
) -> Result<SelectionPage, ServiceError> {
    ensure_run_exists(db, selection_id)?;
    let (items, total) = db
        .list_selection_source_page(
            selection_id,
            limit.clamp(1, 250),
            offset,
            PageFilters {
                query: filters.query,
                min_size: filters.min_size,
                include_rejected: filters.include_rejected,
            },
        )
        .map_err(service_err)?;
    Ok(SelectionPage { items, total })
}

pub fn list_shortlist_page(
    db: &Database,
    selection_id: &str,
    offset: u32,
    limit: u32,
    filters: SelectionPageFilters,
) -> Result<SelectionPage, ServiceError> {
    ensure_run_exists(db, selection_id)?;
    let (items, total) = db
        .list_selection_shortlist_page(
            selection_id,
            limit.clamp(1, 250),
            offset,
            PageFilters {
                query: filters.query,
                min_size: filters.min_size,
                include_rejected: filters.include_rejected,
            },
        )
        .map_err(service_err)?;
    Ok(SelectionPage { items, total })
}

/// The single reusable shortlist mutation entry point.
///
/// Applies one grouped, atomic, undoable membership change for the exact
/// `image_ids` provided (e.g. the user's captured highlight set, or the IDs
/// approved in an agent proposal review). The whole group is validated against
/// the captured source before anything is written; a validation failure or a
/// database error leaves zero writes and zero undo history. Idempotent no-ops
/// change nothing and write no undo record. Actor provenance is logged to
/// `session_events` (`shortlist_added` / `shortlist_removed`, actor_type
/// `user` or `agent`).
pub fn apply_shortlist_change(
    db: &Database,
    action_manager: &ActionManager,
    selection_id: &str,
    image_ids: &[String],
    direction: ShortlistDirection,
    actor: ShortlistActor,
) -> Result<ShortlistMutationResult, ServiceError> {
    if image_ids.is_empty() {
        return Err(ServiceError::InvalidInput(
            "No images provided for the shortlist change".into(),
        ));
    }
    let action = match direction {
        ShortlistDirection::Add => Action::ShortlistAdd {
            selection_id: selection_id.to_string(),
            image_ids: image_ids.to_vec(),
        },
        ShortlistDirection::Remove => Action::ShortlistRemove {
            selection_id: selection_id.to_string(),
            image_ids: image_ids.to_vec(),
        },
    };
    // ActionManager reports validation and lifecycle rejections as strings;
    // they are always actionable messages, so surface them as invalid input.
    let result = action_manager
        .execute(db, action)
        .map_err(ServiceError::InvalidInput)?;
    let changed = !result.undo_record_id.is_empty();
    if changed {
        log_shortlist_provenance(db, selection_id, direction, image_ids, &actor, &result);
    }
    let state = get_selection_state(db, selection_id)?;
    Ok(ShortlistMutationResult {
        state,
        changed,
        undo_record_id: result.undo_record_id,
        label: result.label,
    })
}

fn log_shortlist_provenance(
    db: &Database,
    selection_id: &str,
    direction: ShortlistDirection,
    image_ids: &[String],
    actor: &ShortlistActor,
    result: &ActionResult,
) {
    let event_type = match direction {
        ShortlistDirection::Add => "shortlist_added",
        ShortlistDirection::Remove => "shortlist_removed",
    };
    let _ = db.log_session_event(&crate::db_core::models::NewSessionEvent {
        session_id: None,
        event_type: event_type.to_string(),
        actor_type: actor.actor_type().to_string(),
        actor_id: actor.actor_id(),
        subject_type: Some("selection_run".to_string()),
        subject_id: Some(selection_id.to_string()),
        payload_json: serde_json::json!({
            "image_ids": image_ids,
            "undo_record_id": result.undo_record_id,
            "label": result.label,
        })
        .to_string(),
    });
}

pub fn finish_selection_run(
    db: &Database,
    selection_id: &str,
) -> Result<SelectionState, ServiceError> {
    db.finish_selection_run(selection_id).map_err(service_err)?;
    get_selection_state(db, selection_id)
}

pub fn reopen_selection_run(
    db: &Database,
    selection_id: &str,
) -> Result<SelectionState, ServiceError> {
    db.reopen_selection_run(selection_id).map_err(service_err)?;
    get_selection_state(db, selection_id)
}

pub fn archive_selection_run(
    db: &Database,
    selection_id: &str,
) -> Result<SelectionState, ServiceError> {
    db.archive_selection_run(selection_id)
        .map_err(service_err)?;
    get_selection_state(db, selection_id)
}

pub fn restore_selection_run(
    db: &Database,
    selection_id: &str,
) -> Result<SelectionState, ServiceError> {
    db.restore_selection_run(selection_id)
        .map_err(service_err)?;
    get_selection_state(db, selection_id)
}

fn ensure_run_exists(db: &Database, selection_id: &str) -> Result<(), ServiceError> {
    let exists = db
        .get_selection_run(selection_id)
        .map_err(service_err)?
        .is_some();
    if !exists {
        return Err(ServiceError::NotFound(format!(
            "Selection run '{selection_id}' was not found"
        )));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::{Image, ImageFile};
    use crate::services::undo::ActionManager;
    use crate::services::undo_history::enrich_undo_history;

    fn open_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    fn seed_image(db: &Database, id: &str) {
        db.insert_image(&Image {
            id: id.to_string(),
            sha256_hash: format!("hash-{id}"),
            width: 800,
            height: 600,
            format: "jpg".to_string(),
            file_size: 1024,
            created_at: "2026-09-01T00:00:00Z".to_string(),
            imported_at: "2026-09-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{id}"),
            image_id: id.to_string(),
            path: format!("/library/shoot/{id}.jpg"),
            last_seen_at: "2026-09-01T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: Some(1024),
            last_seen_mtime: None,
        })
        .unwrap();
    }

    fn folder_scope() -> SelectionSourceScope {
        SelectionSourceScope::Folder {
            path: "/library/shoot".into(),
            min_size: 0,
            include_rejected: false,
        }
    }

    fn setup(ids: &[&str]) -> (Database, ActionManager, String) {
        let db = open_db();
        for id in ids {
            seed_image(&db, id);
        }
        let manager = ActionManager::new();
        let run_id = create_selection_run(&db, "Client final", &folder_scope(), Some(2))
            .unwrap()
            .run
            .id;
        (db, manager, run_id)
    }

    fn ids_of<'a>(result: &'a ShortlistMutationResult) -> &'a [String] {
        &result.state.shortlist_ids
    }

    #[test]
    fn create_starts_empty_and_independent_of_decisions() {
        let (db, _manager, run_id) = setup(&["a", "b", "c"]);
        db.set_decision("a", "accept").unwrap();
        db.set_rating("b", 4).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.run.source_count, 3);
        assert_eq!(state.run.shortlist_count, 0);
        assert!(state.shortlist_ids.is_empty());
        assert_eq!(state.run.target_count, Some(2));
        // Decisions and ratings survive run creation untouched.
        assert_eq!(
            db.get_selection_for_image("a").unwrap().unwrap().decision,
            "accept"
        );
        assert_eq!(
            db.get_selection_for_image("b")
                .unwrap()
                .unwrap()
                .star_rating,
            Some(4)
        );
    }

    #[test]
    fn group_add_is_one_undo_step_and_order_is_addition_order() {
        let (db, manager, run_id) = setup(&["a", "b", "c"]);
        let result = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["c".into(), "a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        assert!(result.changed);
        assert_eq!(ids_of(&result), &["c", "a"]);
        assert_eq!(result.state.run.shortlist_count, 2);

        manager.undo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
        manager.redo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["c", "a"]);
    }

    #[test]
    fn idempotent_no_ops_change_nothing_and_write_no_undo_record() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        let first = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        assert!(first.changed);

        // Re-adding a member: no record, redo branch preserved.
        let noop = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        assert!(!noop.changed);
        assert!(noop.undo_record_id.is_empty());

        // Removing a non-member: no record.
        let noop = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["b".into()],
            ShortlistDirection::Remove,
            ShortlistActor::User,
        )
        .unwrap();
        assert!(!noop.changed);

        // The earlier add is still undoable (redo branch was not clobbered).
        manager.undo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
    }

    #[test]
    fn outside_source_add_rejects_whole_group_without_writes() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        seed_image(&db, "outside-1");
        let error = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "outside-1".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("outside the captured selection source"));
        // No membership, no undo record.
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
        assert_eq!(manager.status(&db).stack_depth, 0);
    }

    #[test]
    fn finished_run_rejects_membership_changes_until_reopened() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        finish_selection_run(&db, &run_id).unwrap();

        let error = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["b".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap_err();
        assert!(error.to_string().contains("reopen"));

        // Undo of the pre-finish add is also rejected while finished.
        let error = manager.undo(&db).unwrap_err();
        assert!(error.contains("reopen or restore"));

        reopen_selection_run(&db, &run_id).unwrap();
        // Membership intact after reopen; membership changes work again.
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["a"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["b".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["a", "b"]);
    }

    #[test]
    fn undo_redo_of_removes_and_never_resurrects_deleted_images() {
        let (db, manager, run_id) = setup(&["a", "b", "c"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "b".into(), "c".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        let removed = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "c".into()],
            ShortlistDirection::Remove,
            ShortlistActor::User,
        )
        .unwrap();
        assert_eq!(ids_of(&removed), &["b"]);

        // Undo the remove: both return in addition order.
        manager.undo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["a", "b", "c"]);

        // Delete image "c" through an independent authorized workflow, then
        // redo the removal: "c" must not resurrect.
        {
            let conn = db.conn.lock();
            conn.execute("DELETE FROM images WHERE id = 'c'", [])
                .unwrap();
        }
        manager.redo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["b"]);
        assert_eq!(state.run.source_count, 2);

        // Undo the add of "a" and "b": "b" remains only.
        manager.undo(&db).unwrap();
        manager.undo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
    }

    #[test]
    fn agent_provenance_is_logged_with_actor_type() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::Agent {
                id: Some("proposal-1".into()),
            },
        )
        .unwrap();
        let conn = db.conn.lock();
        let (event_type, actor_type, actor_id): (String, String, Option<String>) = conn
            .query_row(
                "SELECT event_type, actor_type, actor_id FROM session_events
                 WHERE subject_type = 'selection_run' ORDER BY created_at DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(event_type, "shortlist_added");
        assert_eq!(actor_type, "agent");
        assert_eq!(actor_id.as_deref(), Some("proposal-1"));
    }

    #[test]
    fn undo_record_insert_failure_rolls_back_whole_group() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        {
            let conn = db.conn.lock();
            conn.execute(
                "CREATE TRIGGER fail_undo_insert
                 BEFORE INSERT ON undo_records
                 BEGIN
                     SELECT RAISE(ABORT, 'injected undo record failure');
                 END",
                [],
            )
            .unwrap();
        }
        let error = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "b".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap_err();
        assert!(error.to_string().contains("injected undo record failure"));
        // No membership, no undo record: history stays coherent.
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
        assert_eq!(manager.status(&db).stack_depth, 0);

        // Removing the trigger restores normal operation.
        {
            let conn = db.conn.lock();
            conn.execute("DROP TRIGGER fail_undo_insert", []).unwrap();
        }
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["a"]);
    }

    #[test]
    fn failed_mutation_preserves_existing_undo_history() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();

        // The next mutation fails while staging its undo record.
        {
            let conn = db.conn.lock();
            conn.execute(
                "CREATE TRIGGER fail_undo_insert
                 BEFORE INSERT ON undo_records
                 BEGIN
                     SELECT RAISE(ABORT, 'injected undo record failure');
                 END",
                [],
            )
            .unwrap();
        }
        let error = apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["b".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap_err();
        assert!(error.to_string().contains("injected undo record failure"));
        // The first change is still in place and still undoable.
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["a"]);
        manager.undo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
    }

    #[test]
    fn undo_replay_failure_preserves_membership_atomically() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "b".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "CREATE TRIGGER fail_replay_delete
                 BEFORE DELETE ON collection_items
                 BEGIN
                     SELECT RAISE(ABORT, 'injected replay failure');
                 END",
                [],
            )
            .unwrap();
        }
        let error = manager.undo(&db).unwrap_err();
        assert!(error.to_string().contains("injected replay failure"));
        // Replayed restore is one transaction: membership is unchanged.
        let state = get_selection_state(&db, &run_id).unwrap();
        assert_eq!(state.shortlist_ids, vec!["a", "b"]);
        {
            let conn = db.conn.lock();
            conn.execute("DROP TRIGGER fail_replay_delete", []).unwrap();
        }
        manager.undo(&db).unwrap();
        let state = get_selection_state(&db, &run_id).unwrap();
        assert!(state.shortlist_ids.is_empty());
    }

    #[test]
    fn undo_redo_preserve_manual_edits_after_finish_and_reopen() {
        let (db, manager, run_id) = setup(&["a", "b", "c"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        db.finish_selection_run(&run_id).unwrap();
        db.add_to_collection(&run_id, &["b"]).unwrap();
        db.reopen_selection_run(&run_id).unwrap();
        manager.undo(&db).unwrap();
        assert_eq!(db.selection_shortlist_ids(&run_id).unwrap(), vec!["b"]);
        manager.redo(&db).unwrap();
        assert_eq!(db.selection_shortlist_ids(&run_id).unwrap(), vec!["a", "b"]);
        db.finish_selection_run(&run_id).unwrap();
        db.remove_from_collection(&run_id, "a").unwrap();
        db.reopen_selection_run(&run_id).unwrap();
        assert!(manager
            .undo(&db)
            .unwrap_err()
            .contains("conflicts with later collection edits"));
        assert_eq!(db.selection_shortlist_ids(&run_id).unwrap(), vec!["b"]);
        assert!(
            manager.redo(&db).unwrap().is_none(),
            "failed undo must not enable a resurrection through redo"
        );
    }

    #[test]
    fn undo_remove_preserves_later_removal_and_restores_relative_order() {
        let (db, manager, run_id) = setup(&["a", "b", "c", "d"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "b".into(), "c".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["b".into()],
            ShortlistDirection::Remove,
            ShortlistActor::User,
        )
        .unwrap();
        db.finish_selection_run(&run_id).unwrap();
        db.remove_from_collection(&run_id, "a").unwrap();
        db.add_to_collection(&run_id, &["d"]).unwrap();
        db.reopen_selection_run(&run_id).unwrap();
        manager.undo(&db).unwrap();
        assert_eq!(
            db.selection_shortlist_ids(&run_id).unwrap(),
            vec!["b", "c", "d"]
        );
        manager.redo(&db).unwrap();
        assert_eq!(db.selection_shortlist_ids(&run_id).unwrap(), vec!["c", "d"]);
    }

    #[test]
    fn undo_history_titles_cover_shortlist_actions() {
        let (db, manager, run_id) = setup(&["a", "b"]);
        apply_shortlist_change(
            &db,
            &manager,
            &run_id,
            &["a".into(), "b".into()],
            ShortlistDirection::Add,
            ShortlistActor::User,
        )
        .unwrap();
        let entries = enrich_undo_history(&db, &std::path::PathBuf::from("/tmp"), 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action_title, "Add to shortlist");
        assert_eq!(entries[0].change_summary.as_deref(), Some("2 images"));
    }
}
