use crate::db_core::db::Database;
use crate::db_core::queries::external_assets::PendingExternalImport;
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
const MIN_ESTIMATED_ASSET_BYTES: u64 = 32 * 1024 * 1024;
const IMPORT_DISK_RESERVE_BYTES: u64 = 512 * 1024 * 1024;

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
    InsufficientSpace { required: u64, available: u64 },
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
            Self::InsufficientSpace { .. } => "insufficient_disk_space",
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
            Self::InsufficientSpace { required, available } => write!(
                f,
                "Not enough free disk space for Apple Photos import. Cull needs about {} MB free, but {} MB is available",
                required.div_ceil(1024 * 1024),
                available / (1024 * 1024),
            ),
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

pub trait PhotosDiskSpace {
    fn available_bytes(&self, path: &Path) -> Result<u64, PhotosImportError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemPhotosDiskSpace;

impl PhotosDiskSpace for SystemPhotosDiskSpace {
    fn available_bytes(&self, path: &Path) -> Result<u64, PhotosImportError> {
        #[cfg(target_os = "macos")]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
                PhotosImportError::Io("Apple Photos storage path contains a null byte".into())
            })?;
            let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
            // SAFETY: `path` is a valid, nul-terminated filesystem path and
            // `stats` points to writable storage for one `statvfs` value.
            if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
                return Err(PhotosImportError::Io(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            // SAFETY: a successful `statvfs` call initialized `stats`.
            let stats = unsafe { stats.assume_init() };
            return Ok(u64::from(stats.f_bavail).saturating_mul(stats.f_frsize));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err(PhotosImportError::Unsupported)
        }
    }
}

pub fn preflight_current_import_space(
    disk_space: &impl PhotosDiskSpace,
    app_data_dir: &Path,
    estimated_bytes: u64,
) -> Result<(), PhotosImportError> {
    let required = estimated_bytes.saturating_add(IMPORT_DISK_RESERVE_BYTES);
    let available = disk_space.available_bytes(app_data_dir)?;
    if available < required {
        return Err(PhotosImportError::InsufficientSpace {
            required,
            available,
        });
    }
    Ok(())
}

fn estimate_current_import_bytes(
    provider: &impl PhotosCurrentResourceProvider,
    asset_ids: &[String],
) -> Result<u64, PhotosImportError> {
    let mut estimated = 0_u64;
    for asset_id in asset_ids {
        let asset_estimate = match provider.describe_current(asset_id) {
            Ok(resource) => u64::from(resource.pixel_width)
                .saturating_mul(u64::from(resource.pixel_height))
                .saturating_mul(8)
                .clamp(MIN_ESTIMATED_ASSET_BYTES, MAX_MATERIALIZED_BYTES),
            Err(error @ PhotosImportError::PermissionDenied) => return Err(error),
            Err(_) => MAX_MATERIALIZED_BYTES,
        };
        estimated = estimated.saturating_add(asset_estimate);
    }
    Ok(estimated)
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
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhotosImportReconciliation {
    pub removed_part_files: u32,
    pub recovered_materialized: u32,
    pub reset_requested: u32,
    pub rejected_unsafe: u32,
}

pub fn reconcile_current_imports(
    db: &Database,
    app_data_dir: &Path,
) -> Result<PhotosImportReconciliation, PhotosImportError> {
    let managed_root = app_data_dir.join("imports").join("apple-photos");
    let mut report = PhotosImportReconciliation::default();
    report.removed_part_files = remove_stale_parts(&managed_root)?;

    let pending = db
        .list_materializing_external_imports()
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;
    for item in pending {
        let path = PathBuf::from(&item.managed_path);
        if !path_is_safe_managed_file(&managed_root, &path) {
            db.mark_external_import_item(
                &item.item_id,
                &item.resource_id,
                "failed",
                None,
                None,
                Some("unsafe_managed_path"),
                Some("Managed Apple Photos path escaped its import root"),
            )
            .map_err(|error| PhotosImportError::Database(error.to_string()))?;
            report.rejected_unsafe += 1;
            continue;
        }
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let (hash, bytes) = hash_file(&path)?;
                db.mark_external_import_item(
                    &item.item_id,
                    &item.resource_id,
                    "materialized",
                    Some(&hash),
                    Some(bytes),
                    None,
                    None,
                )
                .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                report.recovered_materialized += 1;
            }
            Ok(_) => {
                db.mark_external_import_item(
                    &item.item_id,
                    &item.resource_id,
                    "failed",
                    None,
                    None,
                    Some("unsafe_managed_path"),
                    Some("Managed Apple Photos resource was not a regular file"),
                )
                .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                report.rejected_unsafe += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                db.mark_external_import_item(
                    &item.item_id,
                    &item.resource_id,
                    "requested",
                    None,
                    None,
                    None,
                    None,
                )
                .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                report.reset_requested += 1;
            }
            Err(error) => return Err(PhotosImportError::Io(error.to_string())),
        }
    }
    Ok(report)
}

fn remove_stale_parts(root: &Path) -> Result<u32, PhotosImportError> {
    match std::fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(PhotosImportError::Io(error.to_string())),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(PhotosImportError::Io(
                "Managed Apple Photos root is not a real directory".into(),
            ))
        }
        Ok(_) => {}
    }
    fn visit(directory: &Path, removed: &mut u32) -> Result<(), PhotosImportError> {
        for entry in std::fs::read_dir(directory)
            .map_err(|error| PhotosImportError::Io(error.to_string()))?
        {
            let entry = entry.map_err(|error| PhotosImportError::Io(error.to_string()))?;
            let file_type = entry
                .file_type()
                .map_err(|error| PhotosImportError::Io(error.to_string()))?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                visit(&entry.path(), removed)?;
            } else if file_type.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("current-") && name.ends_with(".part") {
                    std::fs::remove_file(entry.path())
                        .map_err(|error| PhotosImportError::Io(error.to_string()))?;
                    *removed += 1;
                }
            }
        }
        Ok(())
    }
    let mut removed = 0;
    visit(root, &mut removed)?;
    Ok(removed)
}

fn path_is_safe_managed_file(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return false;
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return false,
        }
    }
    true
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

pub fn create_preflighted_current_import(
    db: &Database,
    jobs: &JobRegistry,
    app_data_dir: &Path,
    provider: &impl PhotosCurrentResourceProvider,
    disk_space: &impl PhotosDiskSpace,
    asset_ids: &[String],
) -> Result<(PhotosImportStarted, CancellationToken), PhotosImportError> {
    let estimated_bytes = estimate_current_import_bytes(provider, asset_ids)?;
    preflight_current_import_space(disk_space, app_data_dir, estimated_bytes)?;
    let total = u32::try_from(asset_ids.len())
        .map_err(|_| PhotosImportError::UnsupportedResource("too many selected assets".into()))?;
    create_current_import(db, jobs, total)
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
    let pending =
        match journal_current_import_selection(db, &started, &unique, source_album_id.as_deref()) {
            Ok(pending) => pending,
            Err(error) => {
                jobs.finish_from_worker(
                    &started.job_id,
                    WorkerTerminalOutcome::Failed(error.to_string()),
                );
                if let Some(snapshot) = jobs.get(&started.job_id) {
                    let _ = db.save_job(&snapshot);
                }
                return Err(error);
            }
        };
    run_current_import_job(
        provider,
        db,
        app_data_dir,
        jobs,
        started,
        cancel,
        pending,
        emit,
    )
}

pub fn journal_current_import_selection(
    db: &Database,
    started: &PhotosImportStarted,
    asset_ids: &[String],
    source_album_id: Option<&str>,
) -> Result<Vec<PendingExternalImport>, PhotosImportError> {
    db.journal_external_import_selection(
        &started.job_id,
        &started.batch_id,
        source_album_id,
        asset_ids,
    )
    .map_err(|error| PhotosImportError::Database(error.to_string()))
}

fn mark_cancelled_items(
    db: &Database,
    pending: &[PendingExternalImport],
) -> Result<(), PhotosImportError> {
    for item in pending {
        db.mark_external_import_item(
            &item.item_id,
            &item.resource_id,
            "cancelled",
            None,
            None,
            Some("cancelled"),
            Some("Apple Photos import cancelled before this item completed"),
        )
        .map_err(|error| PhotosImportError::Database(error.to_string()))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_current_import_job<P, E>(
    provider: &P,
    db: &Database,
    app_data_dir: &Path,
    jobs: &JobRegistry,
    started: PhotosImportStarted,
    cancel: CancellationToken,
    mut pending: Vec<PendingExternalImport>,
    mut emit: E,
) -> Result<PhotosImportSummary, PhotosImportError>
where
    P: PhotosCurrentResourceProvider,
    E: FnMut(&PhotosImportProgress),
{
    let total = pending.len() as u32;
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
        error: None,
    };

    let mut fatal_index = pending.len();
    let work_result: Result<(), PhotosImportError> = (|| {
        for index in 0..pending.len() {
            fatal_index = index;
            let current = index as u32 + 1;
            if cancel.is_cancelled() {
                summary.cancelled = total.saturating_sub(index as u32);
                mark_cancelled_items(db, &pending[index..])?;
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
            if cancel.is_cancelled() {
                summary.cancelled = total.saturating_sub(index as u32);
                mark_cancelled_items(db, &pending[index..])?;
                break;
            }
            let asset_id = pending[index].asset_id.clone();
            let resource = match provider.describe_current(&asset_id) {
                Ok(resource) => resource,
                Err(
                    error @ (PhotosImportError::PermissionDenied | PhotosImportError::Inaccessible),
                ) => {
                    db.mark_external_import_item(
                        &pending[index].item_id,
                        &pending[index].resource_id,
                        "inaccessible",
                        None,
                        None,
                        Some(error.code()),
                        Some(&error.to_string()),
                    )
                    .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                    summary.inaccessible += 1;
                    jobs.update_progress(&started.job_id, current, Some("Asset inaccessible"));
                    continue;
                }
                Err(PhotosImportError::Cancelled) => {
                    summary.cancelled += total.saturating_sub(index as u32);
                    mark_cancelled_items(db, &pending[index..])?;
                    break;
                }
                Err(error) => {
                    db.mark_external_import_item(
                        &pending[index].item_id,
                        &pending[index].resource_id,
                        "failed",
                        None,
                        None,
                        Some(error.code()),
                        Some(&error.to_string()),
                    )
                    .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                    summary.failed += 1;
                    jobs.update_progress(&started.job_id, current, Some("Discovery failed"));
                    continue;
                }
            };
            let fingerprint = version_fingerprint(&resource);
            let final_path = managed_resource_path(app_data_dir, &resource, &fingerprint)?;
            let mut prep = db
                .bind_external_import_descriptor(
                    &pending[index],
                    &fingerprint,
                    resource.modified_at.as_deref(),
                    &resource.filename,
                    &final_path.to_string_lossy(),
                )
                .map_err(|error| PhotosImportError::Database(error.to_string()))?;
            pending[index].resource_id = prep.resource_id.clone();
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
            let (mut actual_path, content_hash, bytes) = match materialized {
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
                        mark_cancelled_items(db, &pending[index + 1..])?;
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

            if resource.modified_at.is_none() {
                let consolidation = db
                    .consolidate_external_resource_by_content(
                        &prep.item_id,
                        &prep.resource_id,
                        &content_hash,
                    )
                    .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                prep.resource_id = consolidation.resource_id.clone();
                pending[index].resource_id = prep.resource_id.clone();
                let canonical_path = PathBuf::from(&consolidation.managed_path);
                let mut existing_image_id = consolidation.existing_image_id;
                if let Some(discarded) = consolidation.discarded_path {
                    let discarded = PathBuf::from(discarded);
                    let canonical_matches = hash_file(&canonical_path)
                        .map(|(hash, _)| hash == content_hash)
                        .unwrap_or(false);
                    let discarded_matches = hash_file(&discarded)
                        .map(|(hash, _)| hash == content_hash)
                        .unwrap_or(false);
                    let managed_root = app_data_dir.join("imports").join("apple-photos");
                    let canonical_safe = path_is_safe_managed_file(&managed_root, &canonical_path)
                        && std::fs::symlink_metadata(&canonical_path)
                            .map(|metadata| metadata.file_type().is_file())
                            .unwrap_or(false);
                    if canonical_matches
                        && canonical_safe
                        && discarded_matches
                        && path_is_safe_managed_file(&managed_root, &discarded)
                        && std::fs::symlink_metadata(&discarded)
                            .map(|metadata| metadata.file_type().is_file())
                            .unwrap_or(false)
                    {
                        let recovery_dir = app_data_dir
                            .join("import-recovery")
                            .join("apple-photos-duplicates");
                        std::fs::create_dir_all(&recovery_dir)
                            .map_err(|error| PhotosImportError::Io(error.to_string()))?;
                        let recovery_path = recovery_dir.join(format!(
                            "duplicate-{}.{}",
                            Uuid::new_v4(),
                            discarded
                                .extension()
                                .and_then(|value| value.to_str())
                                .unwrap_or("bin")
                        ));
                        std::fs::rename(&discarded, &recovery_path)
                            .map_err(|error| PhotosImportError::Io(error.to_string()))?;
                        // Prefer recoverable OS trash cleanup. If unavailable, the
                        // verified duplicate remains quarantined outside managed imports.
                        let _ = trash::delete(&recovery_path);
                        actual_path = canonical_path;
                    } else if discarded_matches
                        && path_is_safe_managed_file(&managed_root, &discarded)
                    {
                        // The journal's canonical file is missing or corrupt. Keep the
                        // freshly verified bytes and let the importer repair its path.
                        db.update_external_resource_location(
                            &prep.resource_id,
                            &discarded.to_string_lossy(),
                            consolidation
                                .discarded_content_type
                                .as_deref()
                                .unwrap_or(&resource.content_type),
                        )
                        .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                        actual_path = discarded;
                        existing_image_id = None;
                    } else {
                        return Err(PhotosImportError::Io(
                            "Refused to trash an unverified Apple Photos duplicate".into(),
                        ));
                    }
                } else {
                    actual_path = canonical_path;
                }
                if let Some(image_id) = existing_image_id {
                    summary.reused += 1;
                    summary.image_ids.push(image_id);
                    jobs.update_progress(&started.job_id, current, Some("Already imported"));
                    continue;
                }
            }

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
                Ok(None) => {
                    match db
                        .get_image_file_by_path(&actual_path.to_string_lossy())
                        .map_err(|error| PhotosImportError::Database(error.to_string()))?
                    {
                        Some(file) => file.image_id,
                        None => {
                            db.mark_external_import_item(
                                &prep.item_id,
                                &prep.resource_id,
                                "skipped",
                                None,
                                None,
                                Some("importer_skipped"),
                                Some("Importer skipped the materialized Photos resource"),
                            )
                            .map_err(|error| PhotosImportError::Database(error.to_string()))?;
                            summary.skipped += 1;
                            jobs.update_progress(&started.job_id, current, Some("Import skipped"));
                            continue;
                        }
                    }
                }
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
                    mark_cancelled_items(db, &pending[index + 1..])?;
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
            // The importer has committed its image/file records. Publish that fact
            // even if the following provenance transaction fails, so the UI can
            // refresh committed images and a retry can finish the materialized row.
            summary.imported += 1;
            summary.image_ids.push(image_id.clone());
            fatal_index = index + 1;
            db.finalize_external_import_item(
                &prep.item_id,
                &prep.resource_id,
                &image_id,
                &started.batch_id,
            )
            .map_err(|error| PhotosImportError::Database(error.to_string()))?;
            jobs.update_progress(&started.job_id, current, Some(&resource.filename));
        }
        fatal_index = pending.len();
        Ok(())
    })();

    if let Err(error) = work_result {
        summary.error = Some(error.to_string());
        let remaining = &pending[fatal_index..];
        for item in remaining {
            let _ = db.mark_external_import_item(
                &item.item_id,
                &item.resource_id,
                "failed",
                None,
                None,
                Some("batch_aborted"),
                Some(&error.to_string()),
            );
        }
        if summary.cancelled == 0 {
            summary.failed = summary.failed.saturating_add(remaining.len() as u32);
        }
    }

    let unique_images: HashSet<&str> = summary.image_ids.iter().map(String::as_str).collect();
    if let Err(error) = db.update_import_batch_count(&started.batch_id, unique_images.len() as u32)
    {
        summary
            .error
            .get_or_insert_with(|| format!("Failed to save partial import count: {error}"));
    }
    let outcome = if summary.cancelled > 0 || cancel.is_cancelled() {
        WorkerTerminalOutcome::Cancelled
    } else if let Some(error) = summary.error.as_ref() {
        WorkerTerminalOutcome::Failed(error.clone())
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
        if let Err(error) = db.save_job(&snapshot) {
            summary
                .error
                .get_or_insert_with(|| format!("Failed to persist terminal job: {error}"));
        }
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

    struct SelectiveDescribeProvider;

    impl PhotosCurrentResourceProvider for SelectiveDescribeProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            match asset_id {
                "inaccessible" => Err(PhotosImportError::Inaccessible),
                "failed" => Err(PhotosImportError::Native("metadata unavailable".into())),
                _ => unreachable!(),
            }
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            _output: &mut std::fs::File,
            _cancel: &CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            unreachable!()
        }
    }

    struct CancelDuringMaterializeProvider;

    impl PhotosCurrentResourceProvider for CancelDuringMaterializeProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            Ok(PhotosCurrentResource {
                asset_id: asset_id.into(),
                filename: "photo.png".into(),
                content_type: "public.png".into(),
                modified_at: Some("2026-08-14T16:00:00Z".into()),
                pixel_width: 1,
                pixel_height: 1,
            })
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            _output: &mut std::fs::File,
            cancel: &CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            cancel.cancel();
            Err(PhotosImportError::Cancelled)
        }
    }

    struct ImporterSkippedProvider;

    impl PhotosCurrentResourceProvider for ImporterSkippedProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            Ok(PhotosCurrentResource {
                asset_id: asset_id.into(),
                filename: "photo.png".into(),
                content_type: "public.png".into(),
                modified_at: Some("2026-08-14T18:00:00Z".into()),
                pixel_width: 1,
                pixel_height: 1,
            })
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            output: &mut std::fs::File,
            _cancel: &CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            output.write_all(b"unsupported representation").unwrap();
            Ok(PhotosMaterializedMetadata {
                content_type: "public.data".into(),
                extension: "txt".into(),
            })
        }
    }

    struct FatalDescriptorProvider {
        materializations: AtomicUsize,
    }

    struct ChangingUnknownProvider {
        materializations: AtomicUsize,
    }

    impl PhotosCurrentResourceProvider for ChangingUnknownProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            Ok(PhotosCurrentResource {
                asset_id: asset_id.into(),
                filename: "changing.png".into(),
                content_type: "public.png".into(),
                modified_at: None,
                pixel_width: 1,
                pixel_height: 1,
            })
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            output: &mut std::fs::File,
            _cancel: &CancellationToken,
            _progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
        ) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
            let sequence = self.materializations.fetch_add(1, Ordering::SeqCst);
            let image = image::RgbaImage::from_pixel(
                1,
                1,
                if sequence == 0 {
                    image::Rgba([255, 0, 0, 255])
                } else {
                    image::Rgba([0, 0, 255, 255])
                },
            );
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            output.write_all(bytes.get_ref()).unwrap();
            Ok(PhotosMaterializedMetadata {
                content_type: "public.png".into(),
                extension: "png".into(),
            })
        }
    }

    impl PhotosCurrentResourceProvider for FatalDescriptorProvider {
        fn describe_current(
            &self,
            asset_id: &str,
        ) -> Result<PhotosCurrentResource, PhotosImportError> {
            Ok(PhotosCurrentResource {
                asset_id: asset_id.into(),
                filename: if asset_id == "fatal" {
                    "unsupported.txt".into()
                } else {
                    "photo.png".into()
                },
                content_type: "public.png".into(),
                modified_at: Some("2026-08-14T19:00:00Z".into()),
                pixel_width: 1,
                pixel_height: 1,
            })
        }

        fn materialize_current(
            &self,
            _resource: &PhotosCurrentResource,
            output: &mut std::fs::File,
            _cancel: &CancellationToken,
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

    fn journal_rows(db: &Database, job_id: &str) -> Vec<(String, String, Option<String>)> {
        let conn = db.conn.lock();
        let mut statement = conn
            .prepare(
                "SELECT a.provider_asset_id, i.state, i.error_code
                 FROM external_import_items i
                 JOIN external_asset_resources r ON r.id = i.resource_id
                 JOIN external_asset_versions v ON v.id = r.version_id
                 JOIN external_assets a ON a.id = v.external_asset_id
                 WHERE i.job_id = ?1 ORDER BY i.ordinal",
            )
            .unwrap();
        statement
            .query_map([job_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn regular_file_count(root: &Path) -> usize {
        if !root.exists() {
            return 0;
        }
        std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| !entry.file_type().unwrap().is_symlink())
            .map(|entry| {
                if entry.file_type().unwrap().is_dir() {
                    regular_file_count(&entry.path())
                } else {
                    usize::from(entry.file_type().unwrap().is_file())
                }
            })
            .sum()
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

    struct FixedDiskSpace(u64);

    impl PhotosDiskSpace for FixedDiskSpace {
        fn available_bytes(&self, _path: &Path) -> Result<u64, PhotosImportError> {
            Ok(self.0)
        }
    }

    #[test]
    fn insufficient_disk_space_rejects_the_batch_before_job_or_journal_creation() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();

        let error = create_preflighted_current_import(
            &db,
            &jobs,
            &app_data,
            &FakeProvider {
                materializations: AtomicUsize::new(0),
                modified_at: Some("2026-08-15T00:00:00Z".into()),
            },
            &FixedDiskSpace(575 * 1024 * 1024),
            &["asset-one".into(), "asset-two".into()],
        )
        .unwrap_err();

        assert!(matches!(error, PhotosImportError::InsufficientSpace { .. }));
        assert!(error.to_string().contains("Not enough free disk space"));
        assert!(jobs.list().is_empty());
        let conn = db.conn.lock();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM import_batches", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0,
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM external_import_items", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            0,
        );
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
        let (versions, resources): (i64, i64) = db
            .conn
            .lock()
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM external_asset_versions v
                    JOIN external_assets a ON a.id = v.external_asset_id
                    WHERE a.provider_asset_id = 'opaque-id'),
                   (SELECT COUNT(*) FROM external_asset_resources r
                    JOIN external_asset_versions v ON v.id = r.version_id
                    JOIN external_assets a ON a.id = v.external_asset_id
                    WHERE a.provider_asset_id = 'opaque-id')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((versions, resources), (1, 1));
        assert_eq!(
            regular_file_count(&app_data.join("imports/apple-photos")),
            1
        );
    }

    #[test]
    fn unknown_date_keeps_distinct_content_versions_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let provider = ChangingUnknownProvider {
            materializations: AtomicUsize::new(0),
        };

        let first = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["changing-id".into()],
            None,
            |_| {},
        )
        .unwrap();
        let second = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["changing-id".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_ne!(first.image_ids, second.image_ids);
        let (versions, resources): (i64, i64) = db
            .conn
            .lock()
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM external_asset_versions v
                    JOIN external_assets a ON a.id = v.external_asset_id
                    WHERE a.provider_asset_id = 'changing-id'),
                   (SELECT COUNT(*) FROM external_asset_resources r
                    JOIN external_asset_versions v ON v.id = r.version_id
                    JOIN external_assets a ON a.id = v.external_asset_id
                    WHERE a.provider_asset_id = 'changing-id')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((versions, resources), (2, 2));
        assert_eq!(
            regular_file_count(&app_data.join("imports/apple-photos")),
            2
        );
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

    #[test]
    fn retry_finalizes_when_import_committed_before_provenance_link() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let provider = FakeProvider {
            materializations: AtomicUsize::new(0),
            modified_at: Some("2026-08-14T14:00:00Z".into()),
        };
        let resource = provider.describe_current("crash-gap-id").unwrap();
        let fingerprint = version_fingerprint(&resource);
        let path = managed_resource_path(&app_data, &resource, &fingerprint).unwrap();
        let abandoned_batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let abandoned = db
            .prepare_external_import_item(
                "abandoned-job",
                &abandoned_batch,
                0,
                None,
                &resource.asset_id,
                &fingerprint,
                resource.modified_at.as_deref(),
                &resource.filename,
                &path.to_string_lossy(),
            )
            .unwrap();
        let (path, hash, bytes) = materialize_durable(
            &provider,
            &resource,
            &path,
            &CancellationToken::new(),
            |_, _, _| {},
            |_, _| Ok(()),
        )
        .unwrap();
        db.mark_external_import_item(
            &abandoned.item_id,
            &abandoned.resource_id,
            "materialized",
            Some(&hash),
            Some(bytes),
            None,
            None,
        )
        .unwrap();
        let committed_image = crate::db_core::import::import_file(&db, &path, &app_data)
            .unwrap()
            .unwrap();
        // Simulate process death before finalize_external_import_item.

        let recovered = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["crash-gap-id".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_eq!(recovered.image_ids, [committed_image]);
        assert_eq!(provider.materializations.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn startup_reconciliation_removes_parts_and_resets_missing_materializations() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let managed_root = app_data.join("imports/apple-photos");
        let resource_dir = managed_root.join("asset/version");
        std::fs::create_dir_all(&resource_dir).unwrap();
        let part = resource_dir.join("current-stale.part");
        std::fs::write(&part, b"partial").unwrap();
        let final_path = resource_dir.join("photo.png");
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let pending = db
            .prepare_external_import_item(
                "stale-job",
                &batch,
                0,
                None,
                "stale-id",
                "stale-version",
                Some("2026-08-14T12:00:00Z"),
                "photo.png",
                &final_path.to_string_lossy(),
            )
            .unwrap();
        db.mark_external_import_item(
            &pending.item_id,
            &pending.resource_id,
            "materializing",
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let recovered_path = resource_dir.join("recovered.png");
        std::fs::write(&recovered_path, b"complete current representation").unwrap();
        let recovered = db
            .prepare_external_import_item(
                "recoverable-job",
                &batch,
                1,
                None,
                "recoverable-id",
                "recoverable-version",
                Some("2026-08-14T12:01:00Z"),
                "recovered.png",
                &recovered_path.to_string_lossy(),
            )
            .unwrap();
        db.mark_external_import_item(
            &recovered.item_id,
            &recovered.resource_id,
            "materializing",
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let report = reconcile_current_imports(&db, &app_data).unwrap();

        assert_eq!(report.removed_part_files, 1);
        assert_eq!(report.reset_requested, 1);
        assert_eq!(report.recovered_materialized, 1);
        assert!(!part.exists());
        let states: (String, String) = db
            .conn
            .lock()
            .query_row(
                "SELECT i.state, r.state FROM external_import_items i
                 JOIN external_asset_resources r ON r.id = i.resource_id
                 WHERE i.id = ?1",
                [&pending.item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(states, ("requested".into(), "requested".into()));
        let recovered_states: (String, String) = db
            .conn
            .lock()
            .query_row(
                "SELECT i.state, r.state FROM external_import_items i
                 JOIN external_asset_resources r ON r.id = i.resource_id
                 WHERE i.id = ?1",
                [&recovered.item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            recovered_states,
            ("materialized".into(), "materialized".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn startup_reconciliation_never_follows_a_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let managed_root = app_data.join("imports/apple-photos");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&managed_root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let outside_part = outside.join("current-must-survive.part");
        std::fs::write(&outside_part, b"private outside data").unwrap();
        std::fs::write(outside.join("photo.png"), b"not an image").unwrap();
        symlink(&outside, managed_root.join("escape")).unwrap();

        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let pending = db
            .prepare_external_import_item(
                "unsafe-job",
                &batch,
                0,
                None,
                "unsafe-id",
                "unsafe-version",
                Some("2026-08-14T12:00:00Z"),
                "photo.png",
                &managed_root.join("escape/photo.png").to_string_lossy(),
            )
            .unwrap();
        db.mark_external_import_item(
            &pending.item_id,
            &pending.resource_id,
            "materializing",
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let report = reconcile_current_imports(&db, &app_data).unwrap();

        assert_eq!(report.removed_part_files, 0);
        assert_eq!(report.rejected_unsafe, 1);
        assert_eq!(
            std::fs::read(&outside_part).unwrap(),
            b"private outside data"
        );
        let error_code: Option<String> = db
            .conn
            .lock()
            .query_row(
                "SELECT error_code FROM external_import_items WHERE id = ?1",
                [&pending.item_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(error_code.as_deref(), Some("unsafe_managed_path"));
    }

    #[test]
    fn frozen_selection_journals_discovery_failures_with_reason_codes() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();

        let summary = run_current_import(
            &SelectiveDescribeProvider,
            &db,
            &app_data,
            &jobs,
            vec!["inaccessible".into(), "failed".into()],
            Some("album-one".into()),
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.inaccessible, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(
            journal_rows(&db, &summary.job_id),
            vec![
                (
                    "inaccessible".into(),
                    "inaccessible".into(),
                    Some("asset_inaccessible".into())
                ),
                (
                    "failed".into(),
                    "failed".into(),
                    Some("photos_native_error".into())
                ),
            ]
        );
    }

    #[test]
    fn cancellation_before_work_marks_every_frozen_item_cancelled() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let ids = vec!["one".into(), "two".into(), "three".into()];
        let (started, cancel) = create_current_import(&db, &jobs, ids.len() as u32).unwrap();
        let pending = journal_current_import_selection(&db, &started, &ids, None).unwrap();
        cancel.cancel();
        let job_id = started.job_id.clone();

        let summary = run_current_import_job(
            &FakeProvider {
                materializations: AtomicUsize::new(0),
                modified_at: Some("2026-08-14T16:00:00Z".into()),
            },
            &db,
            &app_data,
            &jobs,
            started,
            cancel,
            pending,
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.cancelled, 3);
        assert!(journal_rows(&db, &job_id)
            .iter()
            .all(|(_, state, code)| state == "cancelled" && code.as_deref() == Some("cancelled")));
    }

    #[test]
    fn cancellation_between_items_preserves_completed_item_for_retry() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let provider = FakeProvider {
            materializations: AtomicUsize::new(0),
            modified_at: Some("2026-08-14T17:00:00Z".into()),
        };
        let ids = vec!["one".into(), "two".into(), "three".into()];
        let (started, cancel) = create_current_import(&db, &jobs, ids.len() as u32).unwrap();
        let pending = journal_current_import_selection(&db, &started, &ids, None).unwrap();
        let cancel_from_event = cancel.clone();
        let first_job_id = started.job_id.clone();
        let first = run_current_import_job(
            &provider,
            &db,
            &app_data,
            &jobs,
            started,
            cancel,
            pending,
            |progress| {
                if progress.phase == "discovery" && progress.current == 2 {
                    cancel_from_event.cancel();
                }
            },
        )
        .unwrap();
        assert_eq!(first.imported, 1);
        assert_eq!(first.cancelled, 2);
        let first_rows = journal_rows(&db, &first_job_id);
        assert_eq!(first_rows[0].1, "imported");
        assert_eq!(first_rows[1].1, "cancelled");
        assert_eq!(first_rows[2].1, "cancelled");

        let retry =
            run_current_import(&provider, &db, &app_data, &jobs, ids, None, |_| {}).unwrap();
        assert_eq!(retry.reused, 1);
        assert_eq!(retry.imported, 2);
        assert_eq!(provider.materializations.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn cancellation_during_materialization_marks_current_and_unstarted_items() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();

        let summary = run_current_import(
            &CancelDuringMaterializeProvider,
            &db,
            &app_data,
            &jobs,
            vec!["one".into(), "two".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.cancelled, 2);
        assert!(journal_rows(&db, &summary.job_id)
            .iter()
            .all(|(_, state, code)| state == "cancelled" && code.as_deref() == Some("cancelled")));
    }

    #[test]
    fn importer_skip_is_a_durable_item_outcome() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();

        let summary = run_current_import(
            &ImporterSkippedProvider,
            &db,
            &app_data,
            &jobs,
            vec!["skipped".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.skipped, 1);
        assert_eq!(
            journal_rows(&db, &summary.job_id),
            vec![(
                "skipped".into(),
                "skipped".into(),
                Some("importer_skipped".into())
            )]
        );
    }

    #[test]
    fn fatal_mid_batch_returns_partial_summary_and_marks_remaining_items() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        let jobs = JobRegistry::default();
        let provider = FatalDescriptorProvider {
            materializations: AtomicUsize::new(0),
        };

        let summary = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["completed".into(), "fatal".into(), "unstarted".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.imported, 1);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.image_ids.len(), 1);
        assert!(summary
            .error
            .as_deref()
            .is_some_and(|error| error.contains("Unsupported Photos resource")));
        assert_eq!(provider.materializations.load(Ordering::SeqCst), 1);
        assert_eq!(
            journal_rows(&db, &summary.job_id),
            vec![
                ("completed".into(), "imported".into(), None),
                (
                    "fatal".into(),
                    "failed".into(),
                    Some("batch_aborted".into())
                ),
                (
                    "unstarted".into(),
                    "failed".into(),
                    Some("batch_aborted".into())
                ),
            ]
        );
        assert_eq!(jobs.get(&summary.job_id).unwrap().status, "failed");
    }

    #[test]
    fn provenance_finalize_failure_still_reports_the_committed_image() {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        std::fs::create_dir_all(&app_data).unwrap();
        let db = Database::open(&temp.path().join("test.db")).unwrap();
        db.conn
            .lock()
            .execute_batch(
                "CREATE TRIGGER fail_external_finalize
                 BEFORE UPDATE ON external_asset_resources
                 WHEN NEW.state = 'imported'
                 BEGIN SELECT RAISE(ABORT, 'simulated finalize failure'); END;",
            )
            .unwrap();
        let jobs = JobRegistry::default();
        let provider = FakeProvider {
            materializations: AtomicUsize::new(0),
            modified_at: Some("2026-08-14T20:00:00Z".into()),
        };

        let summary = run_current_import(
            &provider,
            &db,
            &app_data,
            &jobs,
            vec!["committed-before-finalize".into()],
            None,
            |_| {},
        )
        .unwrap();

        assert_eq!(summary.imported, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.image_ids.len(), 1);
        assert!(summary
            .error
            .as_deref()
            .is_some_and(|error| error.contains("simulated finalize failure")));
        assert_eq!(
            db.get_images_by_ids(&[&summary.image_ids[0]])
                .unwrap()
                .len(),
            1
        );
        assert_eq!(journal_rows(&db, &summary.job_id)[0].1, "materialized");
        assert_eq!(jobs.get(&summary.job_id).unwrap().status, "failed");
    }
}
