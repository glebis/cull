use crate::db_core::db::Database;
use crate::db_core::models::{ReferencedSource, ReferencedSourceKind};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub type MountedSourceKind = ReferencedSourceKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountedSource {
    pub platform_volume_id: Option<String>,
    pub display_name: String,
    pub mount_path: PathBuf,
    pub kind: MountedSourceKind,
    pub capacity_bytes: Option<u64>,
    pub writable: bool,
}

pub trait MountedSourceProvider: Send + Sync {
    fn list_mounted_sources(&self) -> Result<Vec<MountedSource>, String>;
}

fn include_platform_volume(
    is_root: bool,
    is_internal: bool,
    is_browsable: bool,
    is_removable: bool,
    is_ejectable: bool,
) -> bool {
    !is_root && is_browsable && (!is_internal || is_removable || is_ejectable)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountedSourceRefresh {
    pub online: Vec<ReferencedSource>,
    pub offline_ids: Vec<String>,
}

pub fn refresh_mounted_sources(
    db: &Database,
    provider: &dyn MountedSourceProvider,
) -> Result<MountedSourceRefresh, String> {
    let discovered = provider.list_mounted_sources()?;
    let remembered = db
        .list_referenced_sources()
        .map_err(|err| err.to_string())?;
    let now = Utc::now().to_rfc3339();
    let mut online = Vec::with_capacity(discovered.len());
    let mut observed_ids = HashSet::new();

    for mounted in discovered {
        let existing = mounted
            .platform_volume_id
            .as_deref()
            .and_then(|volume_id| {
                remembered
                    .iter()
                    .find(|source| source.platform_volume_id.as_deref() == Some(volume_id))
            })
            .or_else(|| {
                if mounted.platform_volume_id.is_some() {
                    return None;
                }
                remembered.iter().find(|source| {
                    source.platform_volume_id.is_none()
                        && source.source_kind == mounted.kind
                        && source.last_mount_path.as_deref()
                            == Some(mounted.mount_path.to_string_lossy().as_ref())
                })
            });

        let mut source = existing.cloned().unwrap_or_else(|| ReferencedSource {
            id: Uuid::new_v4().to_string(),
            platform_volume_id: mounted.platform_volume_id.clone(),
            display_name: mounted.display_name.clone(),
            last_mount_path: Some(mounted.mount_path.to_string_lossy().to_string()),
            source_kind: mounted.kind.clone(),
            capacity_bytes: mounted.capacity_bytes,
            recursive_default: false,
            settings_json: serde_json::json!({ "writable": mounted.writable }).to_string(),
            last_seen_at: now.clone(),
            offline_at: None,
        });

        let mount_path = mounted.mount_path.to_string_lossy().to_string();
        let settings_json = serde_json::json!({ "writable": mounted.writable }).to_string();
        let is_new = existing.is_none();
        let needs_reconnect = existing.is_some()
            && (source.last_mount_path.as_deref() != Some(mount_path.as_str())
                || source.offline_at.is_some());
        let metadata_changed = source.display_name != mounted.display_name
            || source.source_kind != mounted.kind
            || source.capacity_bytes != mounted.capacity_bytes
            || source.settings_json != settings_json;
        if needs_reconnect {
            db.reconnect_referenced_source(
                &source.id,
                mounted.platform_volume_id.as_deref(),
                &mounted.mount_path,
                &now,
            )
            .map_err(|err| err.to_string())?;
        }
        if is_new || needs_reconnect || metadata_changed {
            source.display_name = mounted.display_name;
            source.last_mount_path = Some(mount_path);
            source.source_kind = mounted.kind;
            source.capacity_bytes = mounted.capacity_bytes;
            source.settings_json = settings_json;
            source.last_seen_at = now.clone();
            source.offline_at = None;
            db.upsert_referenced_source(&source)
                .map_err(|err| err.to_string())?;
        }
        observed_ids.insert(source.id.clone());
        online.push(source);
    }

    let mut offline_ids = Vec::new();
    for mut source in remembered {
        if source.source_kind == ReferencedSourceKind::Folder {
            let available = source
                .last_mount_path
                .as_deref()
                .is_some_and(|path| std::path::Path::new(path).is_dir());
            if available && source.offline_at.is_some() {
                source.offline_at = None;
                source.last_seen_at = now.clone();
                db.upsert_referenced_source(&source)
                    .map_err(|err| err.to_string())?;
            } else if !available {
                if source.offline_at.is_none() {
                    source.offline_at = Some(now.clone());
                    db.upsert_referenced_source(&source)
                        .map_err(|err| err.to_string())?;
                }
                offline_ids.push(source.id);
            }
            continue;
        }
        if observed_ids.contains(&source.id) {
            continue;
        }
        if source.offline_at.is_none() {
            source.offline_at = Some(now.clone());
            db.upsert_referenced_source(&source)
                .map_err(|err| err.to_string())?;
        }
        offline_ids.push(source.id);
    }
    online.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    offline_ids.sort();
    Ok(MountedSourceRefresh {
        online,
        offline_ids,
    })
}

pub struct MountedSourceMonitor {
    cancelled: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MountedSourceMonitor {
    pub fn start(
        db: Database,
        provider: Arc<dyn MountedSourceProvider>,
        interval: Duration,
        on_changed: impl Fn(MountedSourceRefresh) + Send + 'static,
    ) -> Self {
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let thread = std::thread::Builder::new()
            .name("cull-mounted-sources".to_string())
            .spawn(move || {
                let mut last_fingerprint: Option<(Vec<(String, Option<String>)>, Vec<String>)> =
                    None;
                while !worker_cancelled.load(Ordering::Relaxed) {
                    if let Ok(refresh) = refresh_mounted_sources(&db, provider.as_ref()) {
                        let fingerprint = (
                            refresh
                                .online
                                .iter()
                                .map(|source| (source.id.clone(), source.last_mount_path.clone()))
                                .collect(),
                            refresh.offline_ids.clone(),
                        );
                        if last_fingerprint.as_ref() != Some(&fingerprint) {
                            on_changed(refresh.clone());
                            last_fingerprint = Some(fingerprint);
                        }
                    }
                    let slices = (interval.as_millis() / 100).max(1) as usize;
                    for _ in 0..slices {
                        if worker_cancelled.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            })
            .ok();
        Self { cancelled, thread }
    }
}

impl Drop for MountedSourceMonitor {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(target_os = "macos")]
pub struct PlatformMountedSourceProvider;

#[cfg(target_os = "macos")]
impl MountedSourceProvider for PlatformMountedSourceProvider {
    fn list_mounted_sources(&self) -> Result<Vec<MountedSource>, String> {
        use objc2::runtime::AnyObject;
        use objc2_foundation::{
            NSFileManager, NSNumber, NSString, NSURLVolumeIsBrowsableKey,
            NSURLVolumeIsEjectableKey, NSURLVolumeIsInternalKey, NSURLVolumeIsReadOnlyKey,
            NSURLVolumeIsRemovableKey, NSURLVolumeLocalizedNameKey, NSURLVolumeTotalCapacityKey,
            NSURLVolumeUUIDStringKey, NSVolumeEnumerationOptions, NSURL,
        };

        fn value(
            url: &NSURL,
            key: &objc2_foundation::NSURLResourceKey,
        ) -> Option<objc2::rc::Retained<AnyObject>> {
            let mut value = None;
            unsafe { url.getResourceValue_forKey_error(&mut value, key).ok()? };
            value
        }
        fn string_value(url: &NSURL, key: &objc2_foundation::NSURLResourceKey) -> Option<String> {
            value(url, key)?
                .downcast_ref::<NSString>()
                .map(ToString::to_string)
        }
        fn bool_value(url: &NSURL, key: &objc2_foundation::NSURLResourceKey) -> Option<bool> {
            value(url, key)?
                .downcast_ref::<NSNumber>()
                .map(NSNumber::as_bool)
        }
        fn u64_value(url: &NSURL, key: &objc2_foundation::NSURLResourceKey) -> Option<u64> {
            value(url, key)?
                .downcast_ref::<NSNumber>()
                .map(NSNumber::as_u64)
        }

        let manager = NSFileManager::defaultManager();
        let volumes = manager
            .mountedVolumeURLsIncludingResourceValuesForKeys_options(
                None,
                NSVolumeEnumerationOptions::SkipHiddenVolumes,
            )
            .ok_or_else(|| "macOS did not return mounted volumes".to_string())?;
        let mut result = Vec::new();
        for url in volumes.iter() {
            let Some(path) = url.path().map(|path| PathBuf::from(path.to_string())) else {
                continue;
            };
            let removable = bool_value(&url, unsafe { NSURLVolumeIsRemovableKey }).unwrap_or(false);
            let ejectable = bool_value(&url, unsafe { NSURLVolumeIsEjectableKey }).unwrap_or(false);
            if !include_platform_volume(
                path == PathBuf::from("/"),
                bool_value(&url, unsafe { NSURLVolumeIsInternalKey }).unwrap_or(false),
                bool_value(&url, unsafe { NSURLVolumeIsBrowsableKey }).unwrap_or(true),
                removable,
                ejectable,
            ) {
                continue;
            }
            let kind = if removable {
                ReferencedSourceKind::SdCard
            } else if ejectable {
                ReferencedSourceKind::ExternalDrive
            } else {
                ReferencedSourceKind::MountedVolume
            };
            let display_name = string_value(&url, unsafe { NSURLVolumeLocalizedNameKey })
                .or_else(|| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "External volume".to_string());
            result.push(MountedSource {
                platform_volume_id: string_value(&url, unsafe { NSURLVolumeUUIDStringKey }),
                display_name,
                mount_path: path,
                kind,
                capacity_bytes: u64_value(&url, unsafe { NSURLVolumeTotalCapacityKey }),
                writable: !bool_value(&url, unsafe { NSURLVolumeIsReadOnlyKey }).unwrap_or(false),
            });
        }
        Ok(result)
    }
}

#[cfg(not(target_os = "macos"))]
pub struct PlatformMountedSourceProvider;

#[cfg(not(target_os = "macos"))]
impl MountedSourceProvider for PlatformMountedSourceProvider {
    fn list_mounted_sources(&self) -> Result<Vec<MountedSource>, String> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use tempfile::tempdir;

    #[derive(Default)]
    struct FakeProvider {
        sources: Mutex<Vec<MountedSource>>,
    }

    impl FakeProvider {
        fn set(&self, sources: Vec<MountedSource>) {
            *self.sources.lock() = sources;
        }
    }

    impl MountedSourceProvider for FakeProvider {
        fn list_mounted_sources(&self) -> Result<Vec<MountedSource>, String> {
            Ok(self.sources.lock().clone())
        }
    }

    fn mounted(volume_id: &str, path: &str) -> MountedSource {
        MountedSource {
            platform_volume_id: Some(volume_id.to_string()),
            display_name: "UNTITLED".to_string(),
            mount_path: PathBuf::from(path),
            kind: ReferencedSourceKind::SdCard,
            capacity_bytes: Some(64_000_000_000),
            writable: true,
        }
    }

    #[test]
    fn removable_volume_is_included_even_when_macos_reports_it_internal() {
        assert!(include_platform_volume(false, true, true, true, true));
    }

    #[test]
    fn non_removable_internal_volume_stays_excluded() {
        assert!(!include_platform_volume(false, true, true, false, false));
    }

    #[test]
    fn mounted_source_becomes_offline_when_removed() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("cull.db")).unwrap();
        let fake = FakeProvider::default();
        fake.set(vec![mounted("volume-1", "/Volumes/CARD")]);
        let first = refresh_mounted_sources(&db, &fake).unwrap();
        assert_eq!(first.online.len(), 1);
        let source_id = first.online[0].id.clone();

        fake.set(Vec::new());
        let refresh = refresh_mounted_sources(&db, &fake).unwrap();
        assert_eq!(refresh.offline_ids, vec![source_id]);
    }

    #[test]
    fn refresh_reports_a_source_that_was_already_offline() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("cull.db")).unwrap();
        let fake = FakeProvider::default();
        db.upsert_referenced_source(&ReferencedSource {
            id: "already-offline".to_string(),
            platform_volume_id: Some("volume-offline".to_string()),
            display_name: "UNTITLED".to_string(),
            last_mount_path: Some("/Volumes/UNTITLED".to_string()),
            source_kind: ReferencedSourceKind::SdCard,
            capacity_bytes: Some(64_000_000_000),
            recursive_default: false,
            settings_json: "{}".to_string(),
            last_seen_at: "2026-08-31T10:00:00Z".to_string(),
            offline_at: Some("2026-08-31T11:00:00Z".to_string()),
        })
        .unwrap();

        let refresh = refresh_mounted_sources(&db, &fake).unwrap();

        assert_eq!(refresh.offline_ids, vec!["already-offline"]);
    }

    #[test]
    fn same_volume_at_a_new_path_reconnects_existing_source() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("cull.db")).unwrap();
        let fake = FakeProvider::default();
        fake.set(vec![mounted("volume-1", "/Volumes/CARD")]);
        let first = refresh_mounted_sources(&db, &fake).unwrap();
        fake.set(vec![mounted("volume-1", "/Volumes/CARD 1")]);
        let second = refresh_mounted_sources(&db, &fake).unwrap();
        assert_eq!(second.online[0].id, first.online[0].id);
        assert_eq!(
            second.online[0].last_mount_path.as_deref(),
            Some("/Volumes/CARD 1")
        );
    }

    #[test]
    fn different_volume_at_the_same_path_creates_a_distinct_source() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("cull.db")).unwrap();
        let fake = FakeProvider::default();
        fake.set(vec![mounted("volume-1", "/Volumes/CARD")]);
        let first = refresh_mounted_sources(&db, &fake).unwrap();
        fake.set(vec![mounted("volume-2", "/Volumes/CARD")]);
        let second = refresh_mounted_sources(&db, &fake).unwrap();
        assert_ne!(second.online[0].id, first.online[0].id);
        assert_eq!(second.offline_ids, vec![first.online[0].id.clone()]);
    }
}
