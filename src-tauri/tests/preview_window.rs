use cull_lib::preview::window::{
    preview_display_window_spec, PREVIEW_DISPLAY_LABEL, PREVIEW_DISPLAY_TITLE,
};

#[test]
fn preview_display_window_uses_semantic_identity_and_route() {
    let spec = preview_display_window_spec();

    assert_eq!(PREVIEW_DISPLAY_LABEL, "preview-display");
    assert_eq!(PREVIEW_DISPLAY_TITLE, "Cull Preview Display");
    assert_eq!(spec.label, PREVIEW_DISPLAY_LABEL);
    assert_eq!(spec.title, PREVIEW_DISPLAY_TITLE);
    assert_eq!(spec.url, "?previewDisplay=1");
    assert!(
        !spec.url.starts_with("index.html?"),
        "SvelteKit dev serves /index.html?previewDisplay=1 as a 404 route"
    );
}

#[test]
fn preview_display_window_has_external_display_friendly_bounds() {
    let spec = preview_display_window_spec();

    assert!(spec.width >= 1200.0);
    assert!(spec.height >= 800.0);
    assert!(spec.min_width >= 640.0);
    assert!(spec.min_height >= 480.0);
}
