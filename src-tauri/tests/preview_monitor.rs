use cull_lib::preview::window::preview_monitor_key;

#[test]
fn preview_monitor_key_is_stable_for_same_display_geometry() {
    let a = preview_monitor_key(0, Some("Sidecar Display"), 1920, 0, 2732, 2048);
    let b = preview_monitor_key(0, Some("Sidecar Display"), 1920, 0, 2732, 2048);

    assert_eq!(a, b);
    assert!(a.contains("sidecar-display"));
    assert!(a.contains("1920x0"));
    assert!(a.contains("2732x2048"));
}

#[test]
fn preview_monitor_key_uses_index_when_display_name_is_missing() {
    let key = preview_monitor_key(2, None, -1200, 0, 1200, 900);

    assert!(key.starts_with("display-2"));
    assert!(key.contains("-1200x0"));
}
