use tauri::Manager;
use crate::AppState;

pub fn start_mcp_server(app_handle: tauri::AppHandle) {
    let state = app_handle.state::<AppState>();
    let app_data_dir = state.app_data_dir.clone();

    tokio::spawn(async move {
        let sock_path = super::socket::socket_path(&app_data_dir);
        if let Err(e) = super::socket::start_socket_server(app_handle, sock_path).await {
            eprintln!("MCP socket server error: {}", e);
        }
    });
}
