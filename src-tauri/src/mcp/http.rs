use std::net::SocketAddr;
use std::sync::Arc;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::LocalSessionManager,
};
use tauri::Manager;

use crate::AppState;
use crate::services::{ServiceContext, tokens};
use super::auth::AuthContext;
use super::tools::ImageViewMcp;

pub fn start_http_server(
    app_handle: tauri::AppHandle,
    host: String,
    port: u16,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_http_server(app_handle, host, port).await {
            eprintln!("MCP HTTP server error: {}", e);
        }
    });
}

async fn run_http_server(
    app_handle: tauri::AppHandle,
    host: String,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("MCP HTTP server listening on {}", addr);

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let auth_handle = app_handle.clone();

        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                let auth_handle = auth_handle.clone();
                async move {
                    // Extract and validate bearer token
                    let auth_header = req.headers()
                        .get(http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());

                    let bearer = match auth_header {
                        Some(ref h) if h.starts_with("Bearer ") => &h[7..],
                        _ => {
                            return Ok::<_, std::convert::Infallible>(
                                hyper::Response::builder()
                                    .status(401)
                                    .header("WWW-Authenticate", "Bearer")
                                    .body(Full::new(Bytes::from("Unauthorized: Bearer token required")))
                                    .unwrap()
                            );
                        }
                    };

                    let state = auth_handle.state::<AppState>();
                    let ctx = ServiceContext::from_app_state(&state, None);
                    let token = match tokens::validate_token(&ctx, bearer) {
                        Ok(Some(t)) => t,
                        Ok(None) => {
                            return Ok(
                                hyper::Response::builder()
                                    .status(401)
                                    .header("WWW-Authenticate", "Bearer error=\"invalid_token\"")
                                    .body(Full::new(Bytes::from("Unauthorized: Invalid or expired token")))
                                    .unwrap()
                            );
                        }
                        Err(e) => {
                            eprintln!("Token validation error: {}", e);
                            return Ok(
                                hyper::Response::builder()
                                    .status(500)
                                    .body(Full::new(Bytes::from("Internal Server Error")))
                                    .unwrap()
                            );
                        }
                    };

                    eprintln!("MCP HTTP: authenticated as '{}' (role: {}) from {}", token.name, token.role, remote_addr);

                    // Create per-request MCP service with the validated token's auth context
                    let auth = AuthContext::Authenticated(token);
                    let handle_for_mcp = auth_handle.clone();
                    let config = StreamableHttpServerConfig::default()
                        .with_stateful_mode(false)
                        .with_json_response(true);
                    let session_manager = Arc::new(LocalSessionManager::default());
                    let mut mcp_service = StreamableHttpService::new(
                        move || Ok(ImageViewMcp::with_auth(handle_for_mcp.clone(), auth.clone())),
                        session_manager,
                        config,
                    );

                    let resp = tower_service::Service::call(&mut mcp_service, req).await.unwrap();

                    // Convert BoxBody response to Full<Bytes>
                    use http_body_util::BodyExt;
                    let (parts, body) = resp.into_parts();
                    let body_bytes = body.collect().await
                        .map(|c| c.to_bytes())
                        .unwrap_or_default();
                    Ok(hyper::Response::from_parts(parts, Full::new(body_bytes)))
                }
            });

            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                eprintln!("MCP HTTP connection error: {}", e);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bearer_extraction() {
        let header = "Bearer tok_abc123secret456";
        assert!(header.starts_with("Bearer "));
        let token = &header[7..];
        assert_eq!(token, "tok_abc123secret456");
    }

    #[test]
    fn test_missing_bearer_prefix() {
        let header = "Basic dXNlcjpwYXNz";
        assert!(!header.starts_with("Bearer "));
    }
}
