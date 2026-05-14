use std::path::PathBuf;

use crate::db_core::db::Database;

use super::CliArgs;

pub struct HeadlessContext {
    pub db: Database,
    pub app_data_dir: PathBuf,
}

impl HeadlessContext {
    pub fn from_args(args: &CliArgs) -> Result<Self, String> {
        let app_data_dir = resolve_app_data_dir(args)?;
        std::fs::create_dir_all(&app_data_dir).map_err(|e| {
            format!(
                "Failed to create app data dir '{}': {}",
                app_data_dir.display(),
                e
            )
        })?;

        let db_path = args
            .db
            .clone()
            .unwrap_or_else(|| app_data_dir.join("cull.db"));
        let db = Database::open(&db_path).map_err(|e| e.to_string())?;

        Ok(Self { db, app_data_dir })
    }
}

fn resolve_app_data_dir(args: &CliArgs) -> Result<PathBuf, String> {
    if let Some(dir) = args.app_data_dir.clone() {
        return Ok(dir);
    }
    dirs::data_dir()
        .map(|dir| dir.join("com.glebkalinin.cull"))
        .ok_or_else(|| "No data dir is available on this system".to_string())
}
