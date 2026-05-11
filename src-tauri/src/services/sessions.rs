use std::path::{Path, PathBuf};
use crate::db_core::models::*;
use crate::services::{ServiceContext, ServiceError};

pub fn sanitize_folder_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

pub fn compute_sha256(path: &Path) -> Result<String, ServiceError> {
    use sha2::{Sha256, Digest};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn validate_file_hash(path: &Path, expected_hash: &str) -> Result<bool, ServiceError> {
    let actual = compute_sha256(path)?;
    Ok(actual == expected_hash)
}

pub fn copy_file_to_session(source: &Path, dest_dir: &Path) -> Result<PathBuf, ServiceError> {
    let filename = source.file_name()
        .ok_or_else(|| ServiceError::InvalidInput("No filename".into()))?;
    let dest = dest_dir.join(filename);
    std::fs::copy(source, &dest)?;
    Ok(dest)
}

pub fn move_file_to_session(source: &Path, dest_dir: &Path) -> Result<PathBuf, ServiceError> {
    let filename = source.file_name()
        .ok_or_else(|| ServiceError::InvalidInput("No filename".into()))?;
    let dest = dest_dir.join(filename);
    if std::fs::rename(source, &dest).is_err() {
        std::fs::copy(source, &dest)?;
        std::fs::remove_file(source)?;
    }
    Ok(dest)
}

pub fn create_session(ctx: &ServiceContext, name: &str, sessions_root: &Path) -> Result<Session, ServiceError> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let folder_name = format!("{}-{}", date, sanitize_folder_name(name));
    let folder_path = sessions_root.join(&folder_name);

    std::fs::create_dir_all(folder_path.join("Imports"))?;
    std::fs::create_dir_all(folder_path.join("Selects"))?;
    std::fs::create_dir_all(folder_path.join("Exports"))?;

    let folder_str = folder_path.to_string_lossy().to_string();
    let id = ctx.db.create_session(name, &folder_str)?;
    ctx.db.get_session(&id).map_err(ServiceError::from)
}

pub fn list_sessions(ctx: &ServiceContext) -> Result<Vec<Session>, ServiceError> {
    Ok(ctx.db.list_sessions()?)
}

pub fn get_session(ctx: &ServiceContext, id: &str) -> Result<Session, ServiceError> {
    Ok(ctx.db.get_session(id)?)
}

pub fn delete_session(ctx: &ServiceContext, id: &str, delete_files: bool) -> Result<(), ServiceError> {
    if delete_files {
        let session = ctx.db.get_session(id)?;
        let folder = Path::new(&session.folder_path);
        if folder.exists() {
            std::fs::remove_dir_all(folder)?;
        }
    }
    Ok(ctx.db.delete_session(id)?)
}

pub fn convert_session_to_collection(ctx: &ServiceContext, id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.convert_session_to_collection(id)?)
}

pub fn validate_session_folder(ctx: &ServiceContext, id: &str) -> Result<bool, ServiceError> {
    let session = ctx.db.get_session(id)?;
    Ok(Path::new(&session.folder_path).exists())
}

pub fn create_canvas(ctx: &ServiceContext, session_id: &str, name: &str, canvas_type: &str) -> Result<Canvas, ServiceError> {
    if canvas_type != "manual" && canvas_type != "query" {
        return Err(ServiceError::InvalidInput(format!("Invalid canvas type: {}", canvas_type)));
    }
    let id = ctx.db.create_canvas(session_id, name, canvas_type)?;
    let canvases = ctx.db.list_canvases(session_id)?;
    canvases.into_iter().find(|c| c.id == id)
        .ok_or_else(|| ServiceError::NotFound("Canvas not found after creation".into()))
}

pub fn list_canvases(ctx: &ServiceContext, session_id: &str) -> Result<Vec<Canvas>, ServiceError> {
    Ok(ctx.db.list_canvases(session_id)?)
}

pub fn update_canvas_layout(ctx: &ServiceContext, canvas_id: &str, layout_json: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.update_canvas_layout(canvas_id, layout_json)?)
}

pub fn delete_canvas(ctx: &ServiceContext, canvas_id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.delete_canvas(canvas_id)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_folder_name() {
        assert_eq!(sanitize_folder_name("Portrait Shoot"), "portrait-shoot");
        assert_eq!(sanitize_folder_name("hello/world:test"), "hello-world-test");
        assert_eq!(sanitize_folder_name("  spaces  "), "spaces");
    }

    #[test]
    fn test_validate_file_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("test.txt");
        std::fs::write(&src, b"hello world").unwrap();
        let expected_hash = compute_sha256(&src).unwrap();
        assert!(validate_file_hash(&src, &expected_hash).unwrap());
        assert!(!validate_file_hash(&src, "wrong_hash").unwrap());
    }

    #[test]
    fn test_copy_file_to_session() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("original.png");
        std::fs::write(&src, b"fake image data").unwrap();
        let dest_dir = tmp.path().join("session/Imports");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let dest = copy_file_to_session(&src, &dest_dir).unwrap();
        assert!(dest.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"fake image data");
        assert!(src.exists());
    }

    #[test]
    fn test_move_file_to_session() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("original.png");
        std::fs::write(&src, b"fake image data").unwrap();
        let dest_dir = tmp.path().join("session/Imports");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let dest = move_file_to_session(&src, &dest_dir).unwrap();
        assert!(dest.exists());
        assert!(!src.exists());
    }
}
