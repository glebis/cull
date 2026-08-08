use serde_json::Value;

use crate::services::curation::{self as curation_service, SetRatingParams};

use super::HeadlessContext;

pub fn set_rating(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: SetRatingParams =
        serde_json::from_value(params).map_err(|e| format!("Invalid set_rating params: {}", e))?;
    let result =
        curation_service::set_rating_in_database(&ctx.db, &parsed).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "status": "ok",
        "image_id": result.image_id(),
        "rating": result.rating(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use tempfile::tempdir;

    fn context() -> (HeadlessContext, tempfile::TempDir) {
        let tmp = tempdir().unwrap();
        let db = Database::open(&tmp.path().join("cull.db")).unwrap();
        let ctx = HeadlessContext {
            db,
            app_data_dir: tmp.path().to_path_buf(),
        };
        (ctx, tmp)
    }

    fn insert_image(db: &Database, id: &str) {
        db.conn.lock().execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
            rusqlite::params![id, format!("hash-{id}")],
        ).unwrap();
    }

    #[test]
    fn set_rating_updates_the_global_selection() {
        let (ctx, _tmp) = context();
        insert_image(&ctx.db, "img1");

        let result =
            set_rating(&ctx, serde_json::json!({ "image_id": "img1", "rating": 4 })).unwrap();

        assert_eq!(result["image_id"], "img1");
        assert_eq!(result["rating"], 4);
        assert_eq!(
            ctx.db
                .get_selection_for_image("img1")
                .unwrap()
                .unwrap()
                .star_rating,
            Some(4)
        );
    }

    #[test]
    fn set_rating_reports_the_canonical_image_id() {
        let (ctx, _tmp) = context();
        insert_image(&ctx.db, "img1");

        let result = set_rating(
            &ctx,
            serde_json::json!({ "image_id": " img1 ", "rating": 3 }),
        )
        .unwrap();

        assert_eq!(result["image_id"], "img1");
        assert_eq!(
            ctx.db
                .get_selection_for_image("img1")
                .unwrap()
                .unwrap()
                .star_rating,
            Some(3)
        );
    }

    #[test]
    fn set_rating_rejects_missing_image() {
        let (ctx, _tmp) = context();
        let error = set_rating(
            &ctx,
            serde_json::json!({ "image_id": "missing", "rating": 5 }),
        )
        .unwrap_err();
        assert!(error.contains("Image 'missing'"), "{error}");
    }

    #[test]
    fn set_rating_rejects_invalid_input_without_writes() {
        let (ctx, _tmp) = context();
        insert_image(&ctx.db, "img1");

        assert!(
            set_rating(&ctx, serde_json::json!({ "image_id": "img1", "rating": 6 }),)
                .unwrap_err()
                .contains("between 0 and 5")
        );
        assert!(
            set_rating(&ctx, serde_json::json!({ "image_id": "", "rating": 3 }),)
                .unwrap_err()
                .contains("valid image ID")
        );
        assert!(ctx.db.get_selection_for_image("img1").unwrap().is_none());
    }
}
