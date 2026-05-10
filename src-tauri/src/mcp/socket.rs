use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::net::UnixListener;
use rmcp::ServiceExt;

use super::tools::ImageViewMcp;

static CONNECTION_COUNT: AtomicU32 = AtomicU32::new(0);

pub fn socket_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("mcp.sock")
}

#[allow(dead_code)]
pub fn active_connections() -> u32 {
    CONNECTION_COUNT.load(Ordering::Relaxed)
}

pub async fn start_socket_server(
    app_handle: tauri::AppHandle,
    sock_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Remove stale socket
    if sock_path.exists() {
        match tokio::net::UnixStream::connect(&sock_path).await {
            Ok(_) => return Err("Another MCP server is already running on this socket".into()),
            Err(_) => { let _ = std::fs::remove_file(&sock_path); }
        }
    }

    let listener = UnixListener::bind(&sock_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))?;
    }

    eprintln!("MCP socket server listening on {:?}", sock_path);

    loop {
        let (stream, _) = listener.accept().await?;
        let handle = app_handle.clone();

        CONNECTION_COUNT.fetch_add(1, Ordering::Relaxed);

        tokio::spawn(async move {
            eprintln!("MCP client connected (active: {})", CONNECTION_COUNT.load(Ordering::Relaxed));

            let mcp = ImageViewMcp::new(handle);
            let (read, write) = tokio::io::split(stream);

            match mcp.serve((read, write)).await {
                Ok(server) => {
                    if let Err(e) = server.waiting().await {
                        eprintln!("MCP session ended with error: {:?}", e);
                    }
                }
                Err(e) => {
                    eprintln!("MCP session failed to start: {:?}", e);
                }
            }

            CONNECTION_COUNT.fetch_sub(1, Ordering::Relaxed);
            eprintln!("MCP client disconnected (active: {})", CONNECTION_COUNT.load(Ordering::Relaxed));
        });
    }
}
