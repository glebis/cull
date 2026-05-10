use tauri::AppHandle;

use crate::dictation;

#[tauri::command]
pub async fn start_dictation(app: AppHandle, locale: Option<String>) -> Result<(), String> {
    let locale = locale.unwrap_or_else(|| "en-US".to_string());
    dictation::start_dictation_native(&app, &locale)
}

#[tauri::command]
pub async fn stop_dictation() -> Result<(), String> {
    dictation::stop_dictation_native()
}
