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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_socket_path_construction() {
        let dir = PathBuf::from("/tmp/test-app-data");
        let path = socket_path(&dir);
        assert_eq!(path, PathBuf::from("/tmp/test-app-data/mcp.sock"));
    }

    #[test]
    fn test_socket_path_nested() {
        let dir = PathBuf::from("/Users/test/Library/Application Support/com.test.app");
        let path = socket_path(&dir);
        assert!(path.ends_with("mcp.sock"));
        assert!(path.to_string_lossy().contains("com.test.app"));
    }

    #[tokio::test]
    async fn test_socket_bind_and_connect() {
        use tokio::time::{timeout, Duration};
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");

        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();

        let client = timeout(Duration::from_secs(5), tokio::net::UnixStream::connect(&sock_path))
            .await.expect("connect timed out").unwrap();
        assert!(client.peer_addr().is_ok());

        let (server_stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await.expect("accept timed out").unwrap();
        assert!(server_stream.peer_addr().is_ok());
    }

    #[tokio::test]
    async fn test_stale_socket_file_not_connectable() {
        use tokio::time::{timeout, Duration};
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("stale.sock");

        std::fs::write(&sock_path, "").unwrap();
        assert!(sock_path.exists());

        let result = timeout(Duration::from_secs(5), tokio::net::UnixStream::connect(&sock_path))
            .await.expect("connect timed out");
        assert!(result.is_err());
    }

    #[test]
    fn test_socket_permissions_0600() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("perm.sock");

        let _listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let meta = std::fs::metadata(&sock_path).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[tokio::test]
    async fn test_multiple_clients_can_connect() {
        use tokio::time::{timeout, Duration};
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("multi.sock");

        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();

        let c1 = timeout(Duration::from_secs(5), tokio::net::UnixStream::connect(&sock_path))
            .await.expect("c1 connect timed out").unwrap();
        let c2 = timeout(Duration::from_secs(5), tokio::net::UnixStream::connect(&sock_path))
            .await.expect("c2 connect timed out").unwrap();

        let (s1, _) = timeout(Duration::from_secs(5), listener.accept())
            .await.expect("s1 accept timed out").unwrap();
        let (s2, _) = timeout(Duration::from_secs(5), listener.accept())
            .await.expect("s2 accept timed out").unwrap();

        assert!(c1.peer_addr().is_ok());
        assert!(c2.peer_addr().is_ok());
        assert!(s1.peer_addr().is_ok());
        assert!(s2.peer_addr().is_ok());
    }
}
