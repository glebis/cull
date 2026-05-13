use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::Mutex;

use super::auth::AuthContext;
use super::tools::CullMcp;
use crate::services::{tokens, ServiceContext};
use crate::AppState;

struct RateLimiter {
    failures: HashMap<IpAddr, (u32, Instant)>,
    max_failures: u32,
    lockout_duration: Duration,
}

impl RateLimiter {
    fn new(max_failures: u32, lockout_duration: Duration) -> Self {
        Self {
            failures: HashMap::new(),
            max_failures,
            lockout_duration,
        }
    }

    fn is_locked_out(&self, ip: &IpAddr) -> bool {
        if let Some((count, first_failure)) = self.failures.get(ip) {
            if *count >= self.max_failures {
                return first_failure.elapsed() < self.lockout_duration;
            }
        }
        false
    }

    fn record_failure(&mut self, ip: IpAddr) {
        let entry = self.failures.entry(ip).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= self.lockout_duration {
            *entry = (1, Instant::now());
        } else {
            entry.0 += 1;
        }
    }

    fn record_success(&mut self, ip: &IpAddr) {
        self.failures.remove(ip);
    }

    fn cleanup(&mut self) {
        let cutoff = self.lockout_duration;
        self.failures
            .retain(|_, (_, first)| first.elapsed() < cutoff);
    }
}

pub fn start_http_server(app_handle: tauri::AppHandle, host: String, port: u16) {
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

    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(
        10,
        Duration::from_secs(15 * 60),
    )));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("MCP HTTP server listening on {}", addr);

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let auth_handle = app_handle.clone();
        let limiter = rate_limiter.clone();

        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                let auth_handle = auth_handle.clone();
                let limiter = limiter.clone();
                let remote_ip = remote_addr.ip();
                async move {
                    // Check rate limit
                    {
                        let mut rl = limiter.lock().await;
                        rl.cleanup();
                        if rl.is_locked_out(&remote_ip) {
                            return Ok::<_, std::convert::Infallible>(
                                hyper::Response::builder()
                                    .status(429)
                                    .header("Retry-After", "900")
                                    .body(Full::new(Bytes::from(
                                        "Too Many Requests: account locked for 15 minutes",
                                    )))
                                    .unwrap(),
                            );
                        }
                    }

                    // Extract and validate bearer token
                    let auth_header = req
                        .headers()
                        .get(http::header::AUTHORIZATION)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());

                    let bearer = match auth_header {
                        Some(ref h) if h.starts_with("Bearer ") => &h[7..],
                        _ => {
                            limiter.lock().await.record_failure(remote_ip);
                            return Ok(hyper::Response::builder()
                                .status(401)
                                .header("WWW-Authenticate", "Bearer")
                                .body(Full::new(Bytes::from(
                                    "Unauthorized: Bearer token required",
                                )))
                                .unwrap());
                        }
                    };

                    let state = auth_handle.state::<AppState>();
                    let ctx = ServiceContext::from_app_state(&state, None);
                    let token = match tokens::validate_token(&ctx, bearer) {
                        Ok(Some(t)) => t,
                        Ok(None) => {
                            limiter.lock().await.record_failure(remote_ip);
                            return Ok(hyper::Response::builder()
                                .status(401)
                                .header("WWW-Authenticate", "Bearer error=\"invalid_token\"")
                                .body(Full::new(Bytes::from(
                                    "Unauthorized: Invalid or expired token",
                                )))
                                .unwrap());
                        }
                        Err(e) => {
                            eprintln!("Token validation error: {}", e);
                            return Ok(hyper::Response::builder()
                                .status(500)
                                .body(Full::new(Bytes::from("Internal Server Error")))
                                .unwrap());
                        }
                    };

                    limiter.lock().await.record_success(&remote_ip);

                    eprintln!(
                        "MCP HTTP: authenticated as '{}' (role: {}) from {}",
                        token.name, token.role, remote_addr
                    );

                    // Create per-request MCP service with the validated token's auth context
                    let auth = AuthContext::Authenticated(token);
                    let handle_for_mcp = auth_handle.clone();
                    let config = StreamableHttpServerConfig::default()
                        .with_stateful_mode(false)
                        .with_json_response(true);
                    let session_manager = Arc::new(LocalSessionManager::default());
                    let mut mcp_service = StreamableHttpService::new(
                        move || Ok(CullMcp::with_auth(handle_for_mcp.clone(), auth.clone())),
                        session_manager,
                        config,
                    );

                    let resp = match tower_service::Service::call(&mut mcp_service, req).await {
                        Ok(r) => r,
                        Err(_) => {
                            return Ok(hyper::Response::builder()
                                .status(500)
                                .body(Full::new(Bytes::from("Internal Server Error")))
                                .unwrap());
                        }
                    };

                    use http_body_util::BodyExt;
                    let (parts, body) = resp.into_parts();
                    let body_bytes = body
                        .collect()
                        .await
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
    use super::*;
    use std::net::Ipv4Addr;

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

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(900));
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        for _ in 0..9 {
            rl.record_failure(ip);
        }
        assert!(!rl.is_locked_out(&ip));
    }

    #[test]
    fn test_rate_limiter_blocks_at_limit() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(900));
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        for _ in 0..10 {
            rl.record_failure(ip);
        }
        assert!(rl.is_locked_out(&ip));
    }

    #[test]
    fn test_rate_limiter_clears_on_success() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(900));
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        for _ in 0..9 {
            rl.record_failure(ip);
        }
        rl.record_success(&ip);
        assert!(!rl.is_locked_out(&ip));
        // Re-failing should start from 0
        for _ in 0..9 {
            rl.record_failure(ip);
        }
        assert!(!rl.is_locked_out(&ip));
    }

    #[test]
    fn test_rate_limiter_different_ips_independent() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(900));
        let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));
        for _ in 0..10 {
            rl.record_failure(ip1);
        }
        assert!(rl.is_locked_out(&ip1));
        assert!(!rl.is_locked_out(&ip2));
    }

    #[test]
    fn test_rate_limiter_cleanup_removes_expired() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(0));
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        rl.record_failure(ip);
        std::thread::sleep(Duration::from_millis(10));
        rl.cleanup();
        assert!(rl.failures.is_empty());
    }
}
