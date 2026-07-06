use cull_lib::preview::state::{
    PreviewDisplayLayout, PreviewDisplayMode, PreviewOverlayConfig, PreviewStateStore,
};
use std::sync::Arc;
use std::thread;

#[test]
fn preview_state_defaults_to_no_image_and_image_only_mode() {
    let store = PreviewStateStore::default();

    let state = store.get();

    assert_eq!(state.image_id, None);
    assert!(state.image_ids.is_empty());
    assert_eq!(state.display_mode, PreviewDisplayMode::ImageOnly);
    assert_eq!(state.layout, PreviewDisplayLayout::Single);
    assert_eq!(state.overlay, PreviewOverlayConfig::default());
    assert!(!state.frozen);
    assert!(!state.blanked);
    assert_eq!(state.version, 0);
}

#[test]
fn preview_state_update_round_trips_image_and_overlay_config() {
    let store = PreviewStateStore::default();
    let overlay = PreviewOverlayConfig {
        show_filename: true,
        show_rating: true,
        show_decision: true,
        show_metadata_rail: false,
        ..PreviewOverlayConfig::default()
    };

    let state = store.update(
        Some("img-42".to_string()),
        Some(vec!["img-42".to_string(), "img-43".to_string()]),
        PreviewDisplayMode::ClientReview,
        Some(PreviewDisplayLayout::Compare),
        overlay,
        Some(true),
        Some(false),
    );

    assert_eq!(state.image_id.as_deref(), Some("img-42"));
    assert_eq!(state.image_ids, vec!["img-42", "img-43"]);
    assert_eq!(state.display_mode, PreviewDisplayMode::ClientReview);
    assert_eq!(state.layout, PreviewDisplayLayout::Compare);
    assert_eq!(state.overlay, overlay);
    assert!(state.frozen);
    assert!(!state.blanked);
    assert_eq!(store.get(), state);
}

#[test]
fn preview_state_version_increments_on_each_update() {
    let store = PreviewStateStore::default();
    let first = store.update(
        Some("img-1".to_string()),
        None,
        PreviewDisplayMode::ImageOnly,
        None,
        PreviewOverlayConfig::default(),
        None,
        None,
    );
    let second = store.update(
        Some("img-2".to_string()),
        None,
        PreviewDisplayMode::ImageOnly,
        None,
        PreviewOverlayConfig::default(),
        Some(false),
        Some(true),
    );

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);
    assert!(second.blanked);
    assert!(second.updated_at_ms >= first.updated_at_ms);
}

#[test]
fn preview_state_supports_concurrent_read_write_access() {
    let store: Arc<PreviewStateStore> = Arc::new(PreviewStateStore::default());
    let writer = {
        let store: Arc<PreviewStateStore> = Arc::clone(&store);
        thread::spawn(move || {
            for index in 0..100 {
                store.update(
                    Some(format!("img-{index}")),
                    None,
                    PreviewDisplayMode::ImageOnly,
                    None,
                    PreviewOverlayConfig::default(),
                    None,
                    None,
                );
            }
        })
    };

    let readers: Vec<_> = (0..4)
        .map(|_| {
            let store: Arc<PreviewStateStore> = Arc::clone(&store);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = store.get();
                }
            })
        })
        .collect();

    writer.join().expect("writer should not panic");
    for reader in readers {
        reader.join().expect("reader should not panic");
    }

    assert_eq!(store.get().version, 100);
}
