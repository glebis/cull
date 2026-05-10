#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{start_dictation_native, stop_dictation_native};

#[cfg(not(target_os = "macos"))]
pub fn start_dictation_native(_app: &tauri::AppHandle, _locale: &str) -> Result<(), String> {
    Err("Native dictation is only available on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn stop_dictation_native() -> Result<(), String> {
    Err("Native dictation is only available on macOS".to_string())
}
