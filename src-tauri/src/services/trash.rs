use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TrashRecord {
    pub original_path: PathBuf,
    pub trashed_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TrashActionState {
    pub image_id: String,
    pub original_path: PathBuf,
    pub trashed_path: PathBuf,
    #[serde(default)]
    pub trashed: bool,
}

pub(crate) trait TrashPlatform {
    fn move_to_trash(&self, source: &Path) -> Result<PathBuf, String>;
}

pub(crate) struct SystemTrash;

#[cfg(target_os = "macos")]
impl TrashPlatform for SystemTrash {
    fn move_to_trash(&self, source: &Path) -> Result<PathBuf, String> {
        use objc2_foundation::{NSFileManager, NSString, NSURL};

        let source = source
            .to_str()
            .ok_or_else(|| "Trash path is not valid UTF-8".to_string())?;
        let source = NSString::from_str(source);
        let source_url = NSURL::fileURLWithPath(&source);
        let mut resulting_url = None;
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&source_url, Some(&mut resulting_url))
            .map_err(|error| error.to_string())?;
        let resulting_url = resulting_url
            .ok_or_else(|| "macOS did not return the Trash destination".to_string())?;
        let resulting_path = resulting_url
            .path()
            .ok_or_else(|| "macOS returned a Trash destination without a file path".to_string())?;
        Ok(PathBuf::from(resulting_path.to_string()))
    }
}

#[cfg(not(target_os = "macos"))]
impl TrashPlatform for SystemTrash {
    fn move_to_trash(&self, _source: &Path) -> Result<PathBuf, String> {
        Err("Reliable Trash and undo are currently supported on macOS only".to_string())
    }
}

pub(crate) fn move_to_trash(
    platform: &dyn TrashPlatform,
    source: &Path,
) -> Result<TrashRecord, String> {
    let trashed_path = platform.move_to_trash(source)?;
    Ok(TrashRecord {
        original_path: source.to_path_buf(),
        trashed_path,
    })
}

pub(crate) fn restore_from_trash(record: &TrashRecord) -> Result<(), String> {
    if record.original_path.exists() && !record.trashed_path.exists() {
        return Ok(());
    }
    rename_exclusive(&record.trashed_path, &record.original_path)
}

pub(crate) fn retrash_exact(record: &TrashRecord) -> Result<(), String> {
    if record.trashed_path.exists() && !record.original_path.exists() {
        return Ok(());
    }
    rename_exclusive(&record.original_path, &record.trashed_path)
}

#[cfg(target_os = "macos")]
fn rename_exclusive(source: &Path, target: &Path) -> Result<(), String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| "Source path contains an invalid NUL byte".to_string())?;
    let target = CString::new(target.as_os_str().as_bytes())
        .map_err(|_| "Target path contains an invalid NUL byte".to_string())?;
    // SAFETY: the paths are valid NUL-terminated strings for the duration of
    // the call. RENAME_EXCL atomically prevents replacing an unrelated file.
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn rename_exclusive(source: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err(format!(
            "Restore target already exists: {}",
            target.display()
        ));
    }
    std::fs::rename(source, target).map_err(|error| error.to_string())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    pub(crate) struct DirectoryTrash {
        root: PathBuf,
        moved: Mutex<Vec<(PathBuf, PathBuf)>>,
    }

    impl DirectoryTrash {
        pub(crate) fn new(root: PathBuf) -> Self {
            Self {
                root,
                moved: Mutex::new(Vec::new()),
            }
        }
    }

    impl TrashPlatform for DirectoryTrash {
        fn move_to_trash(&self, source: &Path) -> Result<PathBuf, String> {
            let file_name = source.file_name().ok_or("missing file name")?;
            let mut destination = self.root.join(file_name);
            let mut suffix = 2;
            while destination.exists() {
                let stem = source
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("file");
                let extension = source.extension().and_then(|value| value.to_str());
                let unique_name = match extension {
                    Some(extension) => format!("{stem} {suffix}.{extension}"),
                    None => format!("{stem} {suffix}"),
                };
                destination = self.root.join(unique_name);
                suffix += 1;
            }
            std::fs::rename(source, &destination).map_err(|error| error.to_string())?;
            self.moved
                .lock()
                .unwrap()
                .push((source.to_path_buf(), destination.clone()));
            Ok(destination)
        }
    }

    #[test]
    fn exact_destination_round_trip_handles_duplicate_names_and_external_trash_roots() {
        let source_volume = tempfile::tempdir().unwrap();
        let trash_volume = tempfile::tempdir().unwrap();
        let first_dir = source_volume.path().join("one");
        let second_dir = source_volume.path().join("two");
        std::fs::create_dir_all(&first_dir).unwrap();
        std::fs::create_dir_all(&second_dir).unwrap();
        let first = first_dir.join("same.png");
        let second = second_dir.join("same.png");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let platform = DirectoryTrash::new(trash_volume.path().to_path_buf());

        let first_record = move_to_trash(&platform, &first).unwrap();
        let second_record = move_to_trash(&platform, &second).unwrap();

        assert_ne!(first_record.trashed_path, second_record.trashed_path);
        assert!(first_record.trashed_path.starts_with(trash_volume.path()));
        assert!(second_record.trashed_path.starts_with(trash_volume.path()));

        restore_from_trash(&first_record).unwrap();
        restore_from_trash(&second_record).unwrap();

        assert_eq!(std::fs::read(&first).unwrap(), b"first");
        assert_eq!(std::fs::read(&second).unwrap(), b"second");
        assert!(!first_record.trashed_path.exists());
        assert!(!second_record.trashed_path.exists());
    }
}
