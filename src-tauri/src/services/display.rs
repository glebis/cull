use crate::commands::deeplink::{emit_open_params, OpenParams};
use crate::services::ServiceError;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{Manager, Runtime};
use tokio::sync::oneshot;

/// How long a display tool waits for the frontend to confirm that it actually
/// performed the navigation. Matches the agent-snapshot round-trip budget; it
/// also has to cover a cold webview draining the replay buffer on init.
pub const NAVIGATION_ACK_TIMEOUT_SECS: u64 = 15;

/// What the frontend reports back after handling an `open-with-params` event
/// that carried a `request_id`.
#[derive(Debug, Clone)]
pub struct NavigationAck {
    pub ok: bool,
    pub error: Option<String>,
}

type AckRegistry = Mutex<HashMap<String, oneshot::Sender<NavigationAck>>>;
static PENDING_NAVIGATION_ACKS: OnceLock<AckRegistry> = OnceLock::new();

fn pending_navigation_acks() -> &'static AckRegistry {
    PENDING_NAVIGATION_ACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn take_navigation_ack(request_id: &str) -> Option<oneshot::Sender<NavigationAck>> {
    pending_navigation_acks()
        .lock()
        .ok()
        .and_then(|mut pending| pending.remove(request_id))
}

/// Resolve a pending navigation. Called from the frontend through the
/// `complete_deep_link_navigation` command. Unknown ids are ignored: a
/// duplicate delivery or a late ack after a timeout is not an error.
pub fn complete_navigation(request_id: &str, ack: NavigationAck) {
    if let Some(sender) = take_navigation_ack(request_id) {
        let _ = sender.send(ack);
    }
}

/// A display tool can only claim success if the *main* window is there to
/// receive the event and can actually be shown. `Emitter::emit` returns `Ok(())`
/// even when zero webviews are listening, so this guard is what stops the tools
/// reporting a success they cannot verify.
///
/// Checking for "main" specifically matters: secondary windows (preview display,
/// detached panels) would otherwise satisfy a bare `webview_windows()` check
/// while the tray window stays hidden.
fn ensure_main_window_visible<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    what: &str,
) -> Result<(), ServiceError> {
    if app_handle.get_webview_window("main").is_none() {
        return Err(ServiceError::Engine(format!(
            "no app window is available to display the {}",
            what
        )));
    }
    crate::try_reveal_main_window(app_handle)
        .map_err(|e| ServiceError::Engine(format!("cannot display the {}: {}", what, e)))
}

/// Removes its request from the ack registry unless disarmed. Without this a
/// dropped future — MCP client disconnect, task shutdown — would leave its
/// sender in the process-global map forever.
struct AckGuard {
    request_id: Option<String>,
}

impl Drop for AckGuard {
    fn drop(&mut self) {
        if let Some(request_id) = self.request_id.take() {
            take_navigation_ack(&request_id);
        }
    }
}

/// Reveal the window, emit the navigation, and wait for the frontend to confirm
/// it. Every display tool goes through here so that a returned `Ok(())` means
/// the UI really navigated — not merely that an event was handed to Tauri.
///
/// Emitting via `emit_open_params` keeps the replay buffer: a navigation fired
/// while the webview is not yet listening is queued, replayed on init, and acked
/// then — as long as that happens inside the timeout.
async fn navigate_and_await_ack<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    mut params: OpenParams,
    what: &str,
) -> Result<(), ServiceError> {
    ensure_main_window_visible(app_handle, what)?;

    let request_id = format!("nav_{}", uuid::Uuid::new_v4().simple());
    let (sender, receiver) = oneshot::channel::<NavigationAck>();
    match pending_navigation_acks().lock() {
        Ok(mut pending) => {
            pending.insert(request_id.clone(), sender);
        }
        Err(_) => {
            return Err(ServiceError::Engine(
                "navigation ack registry is unavailable".to_string(),
            ))
        }
    }
    // Armed from here on. Every exit path below — early return, timeout, or an
    // aborted future — drops the registry entry. Removing an id that was already
    // consumed by the ack is a no-op, so no path needs to clean up by hand.
    let _guard = AckGuard {
        request_id: Some(request_id.clone()),
    };
    params.request_id = Some(request_id);

    if let Err(e) = emit_open_params(app_handle, params) {
        return Err(ServiceError::Engine(e.to_string()));
    }

    match tokio::time::timeout(Duration::from_secs(NAVIGATION_ACK_TIMEOUT_SECS), receiver).await {
        Ok(Ok(ack)) if ack.ok => Ok(()),
        Ok(Ok(ack)) => Err(ServiceError::Engine(format!(
            "the app failed to display the {}: {}",
            what,
            ack.error.as_deref().unwrap_or("unknown error")
        ))),
        Ok(Err(_)) => Err(ServiceError::Engine(format!(
            "the request to display the {} was cancelled before the app confirmed it",
            what
        ))),
        Err(_) => Err(ServiceError::Engine(format!(
            "timed out waiting for the visible app to display the {}",
            what
        ))),
    }
}

pub async fn show_image<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    image_id: &str,
) -> Result<(), ServiceError> {
    let params = OpenParams {
        view: Some("loupe".to_string()),
        image_id: Some(image_id.to_string()),
        ..OpenParams::default()
    };
    navigate_and_await_ack(app_handle, params, "image").await
}

pub async fn navigate_to_folder<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    folder_path: &str,
) -> Result<(), ServiceError> {
    let params = OpenParams {
        folder: Some(folder_path.to_string()),
        view: Some("grid".to_string()),
        ..OpenParams::default()
    };
    navigate_and_await_ack(app_handle, params, "folder").await
}

pub async fn show_collection<R: Runtime>(
    app_handle: &tauri::AppHandle<R>,
    collection_id: &str,
) -> Result<(), ServiceError> {
    let params = OpenParams {
        collection: Some(collection_id.to_string()),
        view: Some("grid".to_string()),
        ..OpenParams::default()
    };
    navigate_and_await_ack(app_handle, params, "collection").await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock app has no webview windows, which is the state the display tools
    /// used to lie about: they emitted into the void and returned `status: ok`.
    #[tokio::test]
    async fn show_collection_errors_when_no_webview_exists() {
        let app = tauri::test::mock_app();
        let err = show_collection(&app.handle().clone(), "col_xyz")
            .await
            .expect_err("show_collection must fail when there is no webview to receive it");
        assert!(
            err.to_string().contains("no app window is available"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn show_image_and_navigate_to_folder_error_when_no_webview_exists() {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        assert!(show_image(&handle, "img_abc123").await.is_err());
        assert!(navigate_to_folder(&handle, "/art/midjourney")
            .await
            .is_err());
    }

    // --- navigation ack round-trip ---

    fn register_ack(request_id: &str) -> oneshot::Receiver<NavigationAck> {
        let (sender, receiver) = oneshot::channel();
        pending_navigation_acks()
            .lock()
            .unwrap()
            .insert(request_id.to_string(), sender);
        receiver
    }

    #[tokio::test]
    async fn complete_navigation_resolves_a_pending_request() {
        let receiver = register_ack("nav_success");

        complete_navigation(
            "nav_success",
            NavigationAck {
                ok: true,
                error: None,
            },
        );

        let ack = receiver.await.expect("ack sender must not be dropped");
        assert!(ack.ok);
        assert!(ack.error.is_none());
        // The request must be consumed, not left behind for a second delivery.
        assert!(!pending_navigation_acks()
            .lock()
            .unwrap()
            .contains_key("nav_success"));
    }

    #[tokio::test]
    async fn complete_navigation_carries_a_frontend_failure() {
        let receiver = register_ack("nav_failure");

        complete_navigation(
            "nav_failure",
            NavigationAck {
                ok: false,
                error: Some("collection not found".to_string()),
            },
        );

        let ack = receiver.await.expect("ack sender must not be dropped");
        assert!(!ack.ok);
        assert_eq!(ack.error.as_deref(), Some("collection not found"));
    }

    #[test]
    fn ack_guard_removes_its_entry_when_dropped() {
        // Models an aborted MCP call: the waiting future is dropped without the
        // ack ever arriving. The registry must not keep the sender forever.
        let _receiver = register_ack("nav_dropped");
        assert!(pending_navigation_acks()
            .lock()
            .unwrap()
            .contains_key("nav_dropped"));

        {
            let _guard = AckGuard {
                request_id: Some("nav_dropped".to_string()),
            };
        }

        assert!(!pending_navigation_acks()
            .lock()
            .unwrap()
            .contains_key("nav_dropped"));
    }

    #[test]
    fn complete_navigation_ignores_unknown_request_ids() {
        // A duplicate delivery or an ack arriving after a timeout must not panic.
        complete_navigation(
            "nav_never_registered",
            NavigationAck {
                ok: true,
                error: None,
            },
        );
    }

    #[test]
    fn test_show_image_payload_structure() {
        let image_id = "img_abc123";
        let payload = serde_json::json!({
            "path": null,
            "paths": null,
            "folder": null,
            "view": "loupe",
            "image_id": image_id,
        });
        assert_eq!(payload["view"], "loupe");
        assert_eq!(payload["image_id"], "img_abc123");
        assert!(payload["path"].is_null());
        assert!(payload["folder"].is_null());
    }

    #[test]
    fn test_navigate_to_folder_payload_structure() {
        let folder = "/art/midjourney";
        let payload = serde_json::json!({
            "folder": folder,
            "view": "grid",
        });
        assert_eq!(payload["view"], "grid");
        assert_eq!(payload["folder"], "/art/midjourney");
    }

    #[test]
    fn test_show_collection_payload_structure() {
        let collection_id = "col_xyz";
        let params = OpenParams {
            collection: Some(collection_id.to_string()),
            view: Some("grid".to_string()),
            ..OpenParams::default()
        };
        let payload = serde_json::to_value(&params).unwrap();
        assert_eq!(payload["collection"], "col_xyz");
        assert_eq!(payload["view"], "grid");
        assert!(payload["folder"].is_null());
    }

    #[test]
    fn test_show_image_payload_with_special_chars() {
        let image_id = "img_with spaces & stuff";
        let payload = serde_json::json!({
            "image_id": image_id,
            "view": "loupe",
        });
        assert_eq!(payload["image_id"], "img_with spaces & stuff");
    }

    #[test]
    fn test_folder_payload_with_unicode_path() {
        let folder = "/Users/gleb/фото/природа";
        let payload = serde_json::json!({
            "folder": folder,
            "view": "grid",
        });
        assert_eq!(payload["folder"], "/Users/gleb/фото/природа");
    }
}
