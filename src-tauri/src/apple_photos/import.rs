use crate::db_core::db::Database;
use crate::services::jobs::{JobRegistry, WorkerTerminalOutcome};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_MATERIALIZED_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct PhotosCurrentResource {
    pub asset_id: String,
    pub filename: String,
    pub content_type: String,
    pub modified_at: Option<String>,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Debug, Clone)]
pub struct PhotosMaterializedMetadata {
    pub content_type: String,
    pub extension: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotosImportError {
    Unsupported,
    PermissionDenied,
    Inaccessible,
    Cancelled,
    UnsupportedResource(String),
    Native(String),
    Io(String),
    Database(String),
    Import(String),
}

impl PhotosImportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported_platform",
            Self::PermissionDenied => "permission_denied",
            Self::Inaccessible => "asset_inaccessible",
            Self::Cancelled => "cancelled",
            Self::UnsupportedResource(_) => "unsupported_resource",
            Self::Native(_) => "photos_native_error",
            Self::Io(_) => "disk_write_failed",
            Self::Database(_) => "database_failed",
            Self::Import(_) => "import_failed",
        }
    }
}

impl fmt::Display for PhotosImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(f, "Apple Photos is unsupported on this platform"),
            Self::PermissionDenied => write!(f, "Apple Photos permission is not granted"),
            Self::Inaccessible => write!(f, "Apple Photos asset is inaccessible"),
            Self::Cancelled => write!(f, "Apple Photos import cancelled"),
            Self::UnsupportedResource(value) => write!(f, "Unsupported Photos resource: {value}"),
            Self::Native(value) | Self::Io(value) | Self::Database(value) | Self::Import(value) => {
                write!(f, "{value}")
            }
        }
    }
}

pub trait PhotosCurrentResourceProvider {
    fn describe_current(&self, asset_id: &str) -> Result<PhotosCurrentResource, PhotosImportError>;
    fn materialize_current(
        &self,
        resource: &PhotosCurrentResource,
        output: &mut File,
        cancel: &CancellationToken,
        progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
    ) -> Result<PhotosMaterializedMetadata, PhotosImportError>;
}

#[derive(Debug, Clone, Serialize)]
pub struct PhotosImportStarted {
    pub job_id: String,
    pub batch_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhotosImportProgress {
    pub job_id: String,
    pub phase: String,
    pub current: u32,
    pub total: u32,
    pub filename: Option<String>,
    pub bytes_current: Option<u64>,
    pub bytes_total: Option<u64>,
    pub fraction: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhotosImportSummary {
    pub job_id: String,
    pub batch_id: String,
    pub imported: u32,
    pub reused: u32,
    pub failed: u32,
    pub skipped: u32,
    pub inaccessible: u32,
    pub cancelled: u32,
    pub image_ids: Vec<String>,
}

pub fn create_current_import(
    db: &Database,
    jobs: &JobRegistry,
    total: u32,
) -> Result<(PhotosImportStarted, CancellationToken), PhotosImportError> {
    let (job_id, cancel) = jobs.create_job("import", total);
    let batch_id = db
        .create_import_batch("apple_photos", 0, None)
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;
    if let Some(snapshot) = jobs.get(&job_id) {
        db.save_job(&snapshot)
            .map_err(|error| PhotosImportError::Database(error.to_string()))?;
    }
    Ok((PhotosImportStarted { job_id, batch_id }, cancel))
}

pub fn run_current_import<P, E>(
    provider: &P,
    db: &Database,
    app_data_dir: &Path,
    jobs: &JobRegistry,
    asset_ids: Vec<String>,
    source_album_id: Option<String>,
    emit: E,
) -> Result<PhotosImportSummary, PhotosImportError>
where
    P: PhotosCurrentResourceProvider,
    E: FnMut(&PhotosImportProgress),
{
    let unique = dedupe_ids(asset_ids);
    let (started, cancel) = create_current_import(db, jobs, unique.len() as u32)?;
    run_current_import_job(
        provider,
        db,
        app_data_dir,
        jobs,
        started,
        cancel,
        unique,
        source_album_id,
        emit,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_current_import_job<P, E>(
    provider: &P,
    db: &Database,
    app_data_dir: &Path,
    jobs: &JobRegistry,
    started: PhotosImportStarted,
    cancel: CancellationToken,
    asset_ids: Vec<String>,
    source_album_id: Option<String>,
    mut emit: E,
) -> Result<PhotosImportSummary, PhotosImportError>
where
    P: PhotosCurrentResourceProvider,
    E: FnMut(&PhotosImportProgress),
{
    let total = asset_ids.len() as u32;
    let mut summary = PhotosImportSummary {
        job_id: started.job_id.clone(),
        batch_id: started.batch_id.clone(),
        imported: 0,
        reused: 0,
        failed: 0,
        skipped: 0,
        inaccessible: 0,
        cancelled: 0,
        image_ids: Vec::new(),
    };

    for (index, asset_id) in asset_ids.iter().enumerate() {
        let current = index as u32 + 1;
        if cancel.is_cancelled() {
            summary.cancelled = total.saturating_sub(index as u32);
            break;
        }
        emit(&PhotosImportProgress {
            job_id: started.job_id.clone(),
            phase: "discovery".into(),
            current,
            total,
            filename: None,
            bytes_current: None,
            bytes_total: None,
            fraction: None,
        });
        let resource = match provider.describe_current(asset_id) {
            Ok(resource) => resource,
            Err(PhotosImportError::PermissionDenied | PhotosImportError::Inaccessible) => {
                summary.inaccessible += 1;
                jobs.update_progress(&started.job_id, current, Some("Asset inaccessible"));
                continue;
            }
            Err(PhotosImportError::Cancelled) => {
                summary.cancelled += total.saturating_sub(index as u32);
                break;
            }
            Err(_) => {
                summary.failed += 1;
                jobs.update_progress(&started.job_id, current, Some("Discovery failed"));
                continue;
            }
        };
        let fingerprint = version_fingerprint(&resource);
        let final_path = managed_resource_path(app_data_dir, &resource, &fingerprint)?;
        let prep = db
            .prepare_external_import_item(
                &started.job_id,
                &started.batch_id,
                index as u32,
                source_album_id.as_deref(),
                asset_id,
                &fingerprint,
                resource.modified_at.as_deref(),
                &resource.filename,
                &final_path.to_string_lossy(),
            )
            .map_err(|error| PhotosImportError::Database(error.to_string()))?;
        // The journal may already point at a rendered current-representation path
        // whose extension differs from the provisional PhotoKit resource name.
        let journal_path = PathBuf::from(&prep.managed_path);

        if let Some(image_id) = prep.existing_image_id {
            summary.reused += 1;
            summary.image_ids.push(image_id);
            jobs.update_progress(&started.job_id, current, Some("Already imported"));
            continue;
        }

        db.mark_external_import_item(
            &prep.item_id,
            &prep.resource_id,
            "materializing",
            None,
            None,
            None,
            None,
        )
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;

        let materialized = materialize_durable(
            provider,
            &resource,
            &journal_path,
            &cancel,
            |done, expected, fraction| {
                emit(&PhotosImportProgress {
                    job_id: started.job_id.clone(),
                    phase: "download".into(),
                    current,
                    total,
                    filename: Some(resource.filename.clone()),
                    bytes_current: done,
                    bytes_total: expected,
                    fraction,
                });
            },
            |actual_path, content_type| {
                db.update_external_resource_location(
                    &prep.resource_id,
                    &actual_path.to_string_lossy(),
                    content_type,
                )
                .map_err(|error| PhotosImportError::Database(error.to_string()))
            },
        );
        let (actual_path, content_hash, bytes) = match materialized {
            Ok(value) => value,
            Err(error) => {
                let state = if matches!(error, PhotosImportError::Cancelled) {
                    "cancelled"
                } else {
                    "failed"
                };
                let _ = db.mark_external_import_item(
                    &prep.item_id,
                    &prep.resource_id,
                    state,
                    None,
                    None,
                    Some(error.code()),
                    Some(&error.to_string()),
                );
                if state == "cancelled" {
                    summary.cancelled += total.saturating_sub(index as u32);
                    break;
                }
                summary.failed += 1;
                jobs.update_progress(&started.job_id, current, Some("Download failed"));
                continue;
            }
        };
        db.mark_external_import_item(
            &prep.item_id,
            &prep.resource_id,
            "materialized",
            Some(&content_hash),
            Some(bytes),
            None,
            None,
        )
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;

        emit(&PhotosImportProgress {
            job_id: started.job_id.clone(),
            phase: "import".into(),
            current,
            total,
            filename: Some(resource.filename.clone()),
            bytes_current: None,
            bytes_total: None,
            fraction: None,
        });
        let imported = crate::db_core::import::import_file_cancellable(
            db,
            &actual_path,
            app_data_dir,
            &|| cancel.is_cancelled(),
        );
        let image_id = match imported {
            Ok(Some(image_id)) => image_id,
            Ok(None) => db
                .get_image_file_by_path(&actual_path.to_string_lossy())
                .map_err(|error| PhotosImportError::Database(error.to_string()))?
                .map(|file| file.image_id)
                .ok_or_else(|| {
                    PhotosImportError::Import(
                        "Importer skipped a materialized Photos resource".into(),
                    )
                })?,
            Err(error) if cancel.is_cancelled() => {
                let _ = db.mark_external_import_item(
                    &prep.item_id,
                    &prep.resource_id,
                    "cancelled",
                    None,
                    None,
                    Some("cancelled"),
                    Some(&error),
                );
                summary.cancelled += total.saturating_sub(index as u32);
                break;
            }
            Err(error) => {
                let _ = db.mark_external_import_item(
                    &prep.item_id,
                    &prep.resource_id,
                    "failed",
                    None,
                    None,
                    Some("import_failed"),
                    Some(&error),
                );
                summary.failed += 1;
                continue;
            }
        };
        db.finalize_external_import_item(
            &prep.item_id,
            &prep.resource_id,
            &image_id,
            &started.batch_id,
        )
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;
        summary.imported += 1;
        summary.image_ids.push(image_id);
        jobs.update_progress(&started.job_id, current, Some(&resource.filename));
    }

    let unique_images: HashSet<&str> = summary.image_ids.iter().map(String::as_str).collect();
    db.update_import_batch_count(&started.batch_id, unique_images.len() as u32)
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;
    let outcome = if summary.cancelled > 0 || cancel.is_cancelled() {
        WorkerTerminalOutcome::Cancelled
    } else if summary.failed > 0 || summary.inaccessible > 0 {
        WorkerTerminalOutcome::Failed(format!(
            "{} Apple Photos item(s) failed",
            summary.failed + summary.inaccessible
        ))
    } else {
        WorkerTerminalOutcome::Completed
    };
    jobs.finish_from_worker(&started.job_id, outcome);
    if let Some(snapshot) = jobs.get(&started.job_id) {
        db.save_job(&snapshot)
            .map_err(|error| PhotosImportError::Database(error.to_string()))?;
    }
    Ok(summary)
}

fn dedupe_ids(asset_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    asset_ids
        .into_iter()
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect()
}

fn version_fingerprint(resource: &PhotosCurrentResource) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"current\0");
    if let Some(modified_at) = resource.modified_at.as_deref() {
        hasher.update(modified_at.as_bytes());
    } else {
        // Unknown provider versions must not falsely reuse a prior edited asset.
        // A nonce forces materialization; byte hashing/import still deduplicates.
        hasher.update(Uuid::new_v4().as_bytes());
    }
    hasher.update(resource.pixel_width.to_le_bytes());
    hasher.update(resource.pixel_height.to_le_bytes());
    hasher.update(resource.content_type.as_bytes());
    hex::encode(hasher.finalize())
}

fn managed_resource_path(
    app_data_dir: &Path,
    resource: &PhotosCurrentResource,
    fingerprint: &str,
) -> Result<PathBuf, PhotosImportError> {
    let mut asset_hash = Sha256::new();
    asset_hash.update(b"apple_photos\0");
    asset_hash.update(resource.asset_id.as_bytes());
    let asset_key = hex::encode(asset_hash.finalize());
    let filename = sanitize_filename(&resource.filename)?;
    Ok(app_data_dir
        .join("imports")
        .join("apple-photos")
        .join(&asset_key[..24])
        .join(&fingerprint[..24])
        .join(filename))
}

fn sanitize_filename(filename: &str) -> Result<String, PhotosImportError> {
    let leaf = Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("current-image");
    let clean: String = leaf
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let extension = Path::new(&clean)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| PhotosImportError::UnsupportedResource(filename.into()))?;
    if !crate::extensions::supported_extensions(true).contains(&extension.as_str())
        || crate::extensions::is_document_extension(&extension)
    {
        return Err(PhotosImportError::UnsupportedResource(extension));
    }
    Ok(clean)
}

fn materialize_durable<P, F, B>(
    provider: &P,
    resource: &PhotosCurrentResource,
    final_path: &Path,
    cancel: &CancellationToken,
    mut progress: F,
    before_persist: B,
) -> Result<(PathBuf, String, u64), PhotosImportError>
where
    P: PhotosCurrentResourceProvider,
    F: FnMut(Option<u64>, Option<u64>, Option<f64>),
    B: FnOnce(&Path, &str) -> Result<(), PhotosImportError>,
{
    if final_path.exists() {
        let (hash, bytes) = hash_file(final_path)?;
        return Ok((final_path.to_path_buf(), hash, bytes));
    }
    let parent = final_path
        .parent()
        .ok_or_else(|| PhotosImportError::Io("Managed Photos path has no parent".into()))?;
    std::fs::create_dir_all(parent).map_err(|error| PhotosImportError::Io(error.to_string()))?;
    let mut part = tempfile::Builder::new()
        .prefix("current-")
        .suffix(".part")
        .tempfile_in(parent)
        .map_err(|error| PhotosImportError::Io(error.to_string()))?;
    let metadata =
        provider.materialize_current(resource, part.as_file_mut(), cancel, &mut progress)?;
    if cancel.is_cancelled() {
        return Err(PhotosImportError::Cancelled);
    }
    part.as_file_mut()
        .sync_all()
        .map_err(|error| PhotosImportError::Io(error.to_string()))?;
    let bytes = part
        .as_file()
        .metadata()
        .map_err(|error| PhotosImportError::Io(error.to_string()))?
        .len();
    if bytes == 0 || bytes > MAX_MATERIALIZED_BYTES {
        return Err(PhotosImportError::Io(format!(
            "Materialized Photos resource has invalid size: {bytes} bytes"
        )));
    }
    let actual_path = final_path.with_extension(&metadata.extension);
    before_persist(&actual_path, &metadata.content_type)?;
    match part.persist_noclobber(&actual_path) {
        Ok(_) => {}
        Err(error) if actual_path.exists() => drop(error.file),
        Err(error) => return Err(PhotosImportError::Io(error.error.to_string())),
    }
    let (hash, bytes) = hash_file(&actual_path)?;
    Ok((actual_path, hash, bytes))
}

fn hash_file(path: &Path) -> Result<(String, u64), PhotosImportError> {
    let mut file = File::open(path).map_err(|error| PhotosImportError::Io(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| PhotosImportError::Io(error.to_string()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    if bytes == 0 || bytes > MAX_MATERIALIZED_BYTES {
        return Err(PhotosImportError::Io(format!(
            "Materialized Photos resource has invalid size: {bytes} bytes"
        )));
    }
    Ok((hex::encode(hasher.finalize()), bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::services::jobs::JobRegistry;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeProvider {
        materializations: AtomicUsize,
        modified_at: Option<String>,
    }

    struct ConvertedCurrentProvider;

    impl PhotosCurrentResourceProvider for ConvertedCurrentProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            Ok(PhotosCurrentResource {
                asset_id: asset_id.into(),
                filename: "edited.heic".into(),
                content_type: "public.heic".into(),
                modified_at: Some("2026-08-14T12:00:00Z".into()),
                pixel_width: 1,
                pixel_height: 1,
            })
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            output: &mut std::fs::File,
            _cancel: &tokio_util::sync::CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            let image = image::DynamicImage::new_rgb8(1, 1);
            let mut bytes = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut bytes, image::ImageFormat::Jpeg)
                .unwrap();
            output.write_all(bytes.get_ref()).unwrap();
            Ok(PhotosMaterializedMetadata {
                content_type: "public.jpeg".into(),
                extension: "jpg".into(),
            })
        }
    }

    struct NeverMaterializesProvider;

    impl PhotosCurrentResourceProvider for NeverMaterializesProvider {
        fn describe_current(
            &self,
            _asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            unreachable!("the retry test already has the resource description")
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            _output: &mut std::fs::File,
            _cancel: &tokio_util::sync::CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            panic!("an existing journaled resource must not be downloaded again")
        }
    }

    impl PhotosCurrentResourceProvider for FakeProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            Ok(PhotosCurrentResource {
                asset_id: asset_id.into(),
                filename: "photo.png".into(),
                content_type: "public.png".into(),
                modified_at: self.modified_at.clone(),
                pixel_width: 1,
                pixel_height: 1,
            })
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            output: &mut std::fs::File,
            _cancel: &tokio_util::sync::CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            self.materializations.fetch_add(1, Ordering::SeqCst);
            let image = image::DynamicImage::new_rgba8(1, 1);
            let mut bytes = std::io::Cursor::new(Vec::new());
            image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
            output.write_all(bytes.get_ref()).unwrap();
            Ok(PhotosMaterializedMetadata {
                content_type: "public.png".into(),
                extension: "png".into(),
            })
        }
    }

    #[test]
    fn current_import_is_durable_and_retry_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let provider = FakeProvider {
            materializations: AtomicUsize::new(0),
            modified_at: Some("2026-08-14T10:00:00Z".into()),
        };

        let first = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["opaque-id".into()],
            None,
            |_| {},
        )
        .unwrap();
        assert_eq!(first.imported, 1);
        assert_eq!(first.failed, 0);
        assert!(std::path::Path::new(
            &db.get_images_by_ids(&[&first.image_ids[0]]).unwrap()[0].path
        )
        .exists());

        let retry = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["opaque-id".into()],
            None,
            |_| {},
        )
        .unwrap();
        assert_eq!(retry.imported, 0);
        assert_eq!(retry.reused, 1);
        assert_eq!(retry.image_ids, first.image_ids);
        assert_eq!(provider.materializations.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn missing_provider_modified_date_forces_materialization_but_deduplicates_equal_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let provider = FakeProvider {
            materializations: AtomicUsize::new(0),
            modified_at: None,
        };

        let first = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["opaque-id".into()],
            None,
            |_| {},
        )
        .unwrap();
        let second = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["opaque-id".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_eq!(provider.materializations.load(Ordering::SeqCst), 2);
        assert_eq!(second.image_ids, first.image_ids);
    }

    #[test]
    fn current_rendered_type_determines_the_durable_extension() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();

        let imported = run_current_import(
            &ConvertedCurrentProvider,
            &db,
            &app_data,
            &jobs,
            vec!["edited-opaque-id".into()],
            None,
            |_| {},
        )
        .unwrap();
        let image = &db.get_images_by_ids(&[&imported.image_ids[0]]).unwrap()[0];

        assert_eq!(
            std::path::Path::new(&image.path).extension().unwrap(),
            "jpg"
        );
        assert!(std::path::Path::new(&image.path).exists());
    }

    #[test]
    fn retry_uses_a_materialized_journal_path_after_rendered_type_changes() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let resource = ConvertedCurrentProvider
            .describe_current("edited-opaque-id")
            .unwrap();
        let fingerprint = version_fingerprint(&resource);
        let provisional = managed_resource_path(&app_data, &resource, &fingerprint).unwrap();
        let batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let first = db
            .prepare_external_import_item(
                "job-one",
                &batch,
                0,
                None,
                &resource.asset_id,
                &fingerprint,
                resource.modified_at.as_deref(),
                &resource.filename,
                &provisional.to_string_lossy(),
            )
            .unwrap();
        let (rendered_path, _, _) = materialize_durable(
            &ConvertedCurrentProvider,
            &resource,
            &provisional,
            &CancellationToken::new(),
            |_, _, _| {},
            |path, content_type| {
                db.update_external_resource_location(
                    &first.resource_id,
                    &path.to_string_lossy(),
                    content_type,
                )
                .map_err(|error| PhotosImportError::Database(error.to_string()))
            },
        )
        .unwrap();

        let retry_batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let retry = db
            .prepare_external_import_item(
                "job-two",
                &retry_batch,
                0,
                None,
                &resource.asset_id,
                &fingerprint,
                resource.modified_at.as_deref(),
                &resource.filename,
                &provisional.to_string_lossy(),
            )
            .unwrap();
        assert_eq!(PathBuf::from(&retry.managed_path), rendered_path);

        materialize_durable(
            &NeverMaterializesProvider,
            &resource,
            &PathBuf::from(retry.managed_path),
            &CancellationToken::new(),
            |_, _, _| {},
            |_, _| Ok(()),
        )
        .unwrap();
    }
}
