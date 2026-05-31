use bytes::{Bytes, BytesMut};
use http_body_util::{BodyExt, Full};
use hyper::body::{Body as HttpBody, Incoming};
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

const MCP_HTTP_MAX_BODY_BYTES: u64 = 1024 * 1024;
const MCP_HTTP_VALID_REQUEST_LIMIT: u32 = 120;
const MCP_HTTP_VALID_REQUEST_WINDOW: Duration = Duration::from_secs(60);
const MCP_HTTP_MAX_CONCURRENT_REQUESTS: usize = 16;
const MCP_HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

use super::auth::AuthContext;
use super::tools::CullMcp;
use crate::services::{tokens, ServiceContext};
use crate::AppState;

#[derive(Debug)]
struct BindPolicy {
    addr: SocketAddr,
    remote_warning: Option<String>,
}

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
        if let Some((count, _)) = self.failures.get_mut(ip) {
            if *count > 1 {
                *count -= 1;
            } else {
                self.failures.remove(ip);
            }
        }
    }

    fn cleanup(&mut self) {
        let cutoff = self.lockout_duration;
        self.failures
            .retain(|_, (_, first)| first.elapsed() < cutoff);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BodySizeDecision {
    Accept,
    RejectTooLarge,
}

fn classify_request_body_size(content_length: Option<u64>, max_bytes: u64) -> BodySizeDecision {
    match content_length {
        Some(length) if length > max_bytes => BodySizeDecision::RejectTooLarge,
        _ => BodySizeDecision::Accept,
    }
}

fn request_content_length(headers: &http::HeaderMap) -> Option<u64> {
    headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

#[derive(Debug, PartialEq, Eq)]
enum RequestBodyError {
    TooLarge,
    ReadFailed,
    TimedOut,
}

async fn read_limited_request_body(
    req: hyper::Request<Incoming>,
    max_bytes: u64,
    timeout: Duration,
) -> Result<hyper::Request<Full<Bytes>>, RequestBodyError> {
    if classify_request_body_size(request_content_length(req.headers()), max_bytes)
        == BodySizeDecision::RejectTooLarge
    {
        return Err(RequestBodyError::TooLarge);
    }

    let (parts, body) = req.into_parts();
    let bytes = collect_limited_body_with_timeout(body, max_bytes, timeout).await?;
    Ok(hyper::Request::from_parts(parts, Full::new(bytes)))
}

async fn collect_limited_body_with_timeout<B>(
    body: B,
    max_bytes: u64,
    timeout: Duration,
) -> Result<Bytes, RequestBodyError>
where
    B: HttpBody<Data = Bytes> + Unpin,
{
    tokio::time::timeout(timeout, collect_limited_body(body, max_bytes))
        .await
        .map_err(|_| RequestBodyError::TimedOut)?
}

async fn collect_limited_body<B>(mut body: B, max_bytes: u64) -> Result<Bytes, RequestBodyError>
where
    B: HttpBody<Data = Bytes> + Unpin,
{
    let mut bytes = BytesMut::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| RequestBodyError::ReadFailed)?;
        if let Ok(chunk) = frame.into_data() {
            if bytes.len() as u64 + chunk.len() as u64 > max_bytes {
                return Err(RequestBodyError::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
    }

    Ok(bytes.freeze())
}

#[derive(Debug, PartialEq, Eq)]
enum ValidRequestDecision {
    Allowed,
    RateLimited { status: u16, retry_after: Duration },
}

struct ValidRequestLimiter {
    by_ip: HashMap<IpAddr, (u32, Instant)>,
    by_token: HashMap<String, (u32, Instant)>,
    max_requests: u32,
    window: Duration,
}

impl ValidRequestLimiter {
    fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            by_ip: HashMap::new(),
            by_token: HashMap::new(),
            max_requests,
            window,
        }
    }

    fn record_request(&mut self, ip: IpAddr, token_id: &str) -> ValidRequestDecision {
        let now = Instant::now();
        self.cleanup(now);

        if self.bucket_is_limited(self.by_ip.get(&ip), now)
            || self.bucket_is_limited(self.by_token.get(token_id), now)
        {
            return ValidRequestDecision::RateLimited {
                status: 429,
                retry_after: self.window,
            };
        }

        Self::increment_bucket(&mut self.by_ip, ip, now, self.window);
        Self::increment_bucket(&mut self.by_token, token_id.to_string(), now, self.window);
        ValidRequestDecision::Allowed
    }

    fn bucket_is_limited(&self, bucket: Option<&(u32, Instant)>, now: Instant) -> bool {
        matches!(bucket, Some((count, start)) if now.duration_since(*start) < self.window && *count >= self.max_requests)
    }

    fn increment_bucket<K: std::cmp::Eq + std::hash::Hash>(
        buckets: &mut HashMap<K, (u32, Instant)>,
        key: K,
        now: Instant,
        window: Duration,
    ) {
        let entry = buckets.entry(key).or_insert((0, now));
        if now.duration_since(entry.1) >= window {
            *entry = (0, now);
        }
        entry.0 += 1;
    }

    fn cleanup(&mut self, now: Instant) {
        let window = self.window;
        self.by_ip
            .retain(|_, (_, start)| now.duration_since(*start) < window);
        self.by_token
            .retain(|_, (_, start)| now.duration_since(*start) < window);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ConcurrencyDecision {
    RateLimited { status: u16 },
}

struct RequestConcurrencyLimiter {
    semaphore: Arc<Semaphore>,
}

impl RequestConcurrencyLimiter {
    fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    fn try_acquire(&self) -> Result<OwnedSemaphorePermit, ConcurrencyDecision> {
        self.semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| ConcurrencyDecision::RateLimited { status: 429 })
    }
}

pub fn start_http_server(
    app_handle: tauri::AppHandle,
    host: String,
    port: u16,
    allow_remote: bool,
) {
    let policy = match resolve_bind_policy(&host, port, allow_remote) {
        Ok(policy) => policy,
        Err(e) => {
            crate::safe_eprintln!("MCP HTTP server not started: {}", e);
            return;
        }
    };

    if let Some(warning) = policy.remote_warning.as_ref() {
        crate::safe_eprintln!("{}", warning);
    }

    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_http_server(app_handle, policy.addr).await {
            crate::safe_eprintln!("MCP HTTP server error: {}", e);
        }
    });
}

async fn run_http_server(
    app_handle: tauri::AppHandle,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new(
        10,
        Duration::from_secs(15 * 60),
    )));
    let valid_request_limiter = Arc::new(Mutex::new(ValidRequestLimiter::new(
        MCP_HTTP_VALID_REQUEST_LIMIT,
        MCP_HTTP_VALID_REQUEST_WINDOW,
    )));
    let concurrency_limiter = Arc::new(RequestConcurrencyLimiter::new(
        MCP_HTTP_MAX_CONCURRENT_REQUESTS,
    ));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    crate::safe_eprintln!("MCP HTTP server listening on {}", addr);

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let auth_handle = app_handle.clone();
        let limiter = rate_limiter.clone();
        let valid_limiter = valid_request_limiter.clone();
        let concurrency_limiter = concurrency_limiter.clone();

        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                let auth_handle = auth_handle.clone();
                let limiter = limiter.clone();
                let valid_limiter = valid_limiter.clone();
                let concurrency_limiter = concurrency_limiter.clone();
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

                    // Reject cross-origin requests
                    if let Some(origin) = req
                        .headers()
                        .get(http::header::ORIGIN)
                        .and_then(|v| v.to_str().ok())
                    {
                        // Browser-originated cross-origin requests include Origin header.
                        // MCP clients (curl, SDKs) typically don't send Origin.
                        // Reject any request with an Origin header as it's likely from a browser.
                        crate::safe_eprintln!(
                            "MCP HTTP: rejecting cross-origin request from origin '{}'",
                            origin
                        );
                        return Ok(hyper::Response::builder()
                            .status(403)
                            .body(Full::new(Bytes::from(
                                "Forbidden: Cross-origin requests are not allowed",
                            )))
                            .unwrap());
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
                            crate::safe_eprintln!("Token validation error: {}", e);
                            return Ok(hyper::Response::builder()
                                .status(500)
                                .body(Full::new(Bytes::from("Internal Server Error")))
                                .unwrap());
                        }
                    };

                    limiter.lock().await.record_success(&remote_ip);

                    match valid_limiter
                        .lock()
                        .await
                        .record_request(remote_ip, token.id.as_str())
                    {
                        ValidRequestDecision::Allowed => {}
                        ValidRequestDecision::RateLimited { retry_after, .. } => {
                            return Ok(hyper::Response::builder()
                                .status(429)
                                .header("Retry-After", retry_after.as_secs().to_string())
                                .body(Full::new(Bytes::from(
                                    "Too Many Requests: valid-token request limit exceeded",
                                )))
                                .unwrap());
                        }
                    }

                    crate::safe_eprintln!(
                        "MCP HTTP: authenticated as '{}' (role: {}) from {}",
                        token.name,
                        token.role,
                        remote_addr
                    );

                    if classify_request_body_size(
                        request_content_length(req.headers()),
                        MCP_HTTP_MAX_BODY_BYTES,
                    ) == BodySizeDecision::RejectTooLarge
                    {
                        return Ok(hyper::Response::builder()
                            .status(413)
                            .body(Full::new(Bytes::from("Payload Too Large")))
                            .unwrap());
                    }

                    let concurrency_permit = match concurrency_limiter.try_acquire() {
                        Ok(permit) => permit,
                        Err(ConcurrencyDecision::RateLimited { .. }) => {
                            return Ok(hyper::Response::builder()
                                .status(429)
                                .body(Full::new(Bytes::from(
                                    "Too Many Requests: MCP HTTP concurrency limit exceeded",
                                )))
                                .unwrap());
                        }
                    };

                    let req = match read_limited_request_body(
                        req,
                        MCP_HTTP_MAX_BODY_BYTES,
                        MCP_HTTP_REQUEST_TIMEOUT,
                    )
                    .await
                    {
                        Ok(req) => req,
                        Err(RequestBodyError::TooLarge) => {
                            return Ok(hyper::Response::builder()
                                .status(413)
                                .body(Full::new(Bytes::from("Payload Too Large")))
                                .unwrap());
                        }
                        Err(RequestBodyError::ReadFailed) => {
                            return Ok(hyper::Response::builder()
                                .status(400)
                                .body(Full::new(Bytes::from("Bad Request")))
                                .unwrap());
                        }
                        Err(RequestBodyError::TimedOut) => {
                            return Ok(hyper::Response::builder()
                                .status(408)
                                .body(Full::new(Bytes::from("Request Timeout")))
                                .unwrap());
                        }
                    };

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

                    let resp = match tokio::time::timeout(
                        MCP_HTTP_REQUEST_TIMEOUT,
                        tower_service::Service::call(&mut mcp_service, req),
                    )
                    .await
                    {
                        Ok(Ok(r)) => r,
                        Ok(Err(_)) => {
                            return Ok(hyper::Response::builder()
                                .status(500)
                                .body(Full::new(Bytes::from("Internal Server Error")))
                                .unwrap());
                        }
                        Err(_) => {
                            return Ok(hyper::Response::builder()
                                .status(504)
                                .body(Full::new(Bytes::from("Gateway Timeout")))
                                .unwrap());
                        }
                    };

                    drop(concurrency_permit);

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
                crate::safe_eprintln!("MCP HTTP connection error: {}", e);
            }
        });
    }
}

fn resolve_bind_policy(host: &str, port: u16, allow_remote: bool) -> Result<BindPolicy, String> {
    let ip: IpAddr = host
        .parse()
        .map_err(|_| format!("invalid MCP HTTP bind host '{}'; use an IP address", host))?;
    let addr = SocketAddr::new(ip, port);
    if !addr.ip().is_loopback() && !allow_remote {
        return Err(format!(
            "non-loopback bind {} requires mcp_http_allow_remote=true or --mcp-http-allow-remote",
            addr
        ));
    }

    let remote_warning = if addr.ip().is_loopback() {
        None
    } else {
        Some(format!(
            "WARNING: MCP HTTP is listening on {}. Use scoped tokens with least privilege before exposing this address.",
            addr
        ))
    };

    Ok(BindPolicy {
        addr,
        remote_warning,
    })
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
    fn test_bind_policy_allows_loopback_without_remote_opt_in() {
        let policy = resolve_bind_policy("127.0.0.1", 9847, false).unwrap();
        assert!(policy.addr.ip().is_loopback());
        assert!(policy.remote_warning.is_none());
    }

    #[test]
    fn test_bind_policy_rejects_non_loopback_without_remote_opt_in() {
        let err = resolve_bind_policy("0.0.0.0", 9847, false).unwrap_err();
        assert!(err.contains("non-loopback"));
        assert!(err.contains("mcp_http_allow_remote"));
    }

    #[test]
    fn test_bind_policy_warns_about_scoped_tokens_for_remote_bind() {
        let policy = resolve_bind_policy("0.0.0.0", 9847, true).unwrap();
        let warning = policy.remote_warning.unwrap();
        assert!(warning.contains("scoped tokens"));
        assert!(warning.contains("least privilege"));
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
    fn test_rate_limiter_decays_on_success_without_clearing_history() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(900));
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        for _ in 0..9 {
            rl.record_failure(ip);
        }
        rl.record_success(&ip);
        assert!(!rl.is_locked_out(&ip));

        let (count, _) = rl
            .failures
            .get(&ip)
            .expect("success should decay, not delete failure history");
        assert_eq!(*count, 8);

        rl.record_failure(ip);
        assert!(!rl.is_locked_out(&ip));
        rl.record_failure(ip);
        assert!(rl.is_locked_out(&ip));
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

    #[test]
    fn test_body_size_rejects_oversized_content_length_before_service_processing() {
        assert_eq!(
            classify_request_body_size(Some(1025), 1024),
            BodySizeDecision::RejectTooLarge
        );
        assert_eq!(
            classify_request_body_size(Some(1024), 1024),
            BodySizeDecision::Accept
        );
    }

    #[tokio::test]
    async fn test_streaming_body_rejects_bytes_past_limit_without_content_length() {
        let result = collect_limited_body(Full::new(Bytes::from_static(b"12345")), 4).await;

        assert_eq!(result.unwrap_err(), RequestBodyError::TooLarge);
    }

    #[tokio::test]
    async fn test_body_read_times_out_instead_of_holding_concurrency_permit() {
        let body = http_body_util::StreamBody::new(futures_util::stream::pending::<
            Result<hyper::body::Frame<Bytes>, std::convert::Infallible>,
        >());

        let result = collect_limited_body_with_timeout(body, 1024, Duration::from_millis(1)).await;

        assert_eq!(result.unwrap_err(), RequestBodyError::TimedOut);
    }

    #[test]
    fn test_valid_request_limiter_returns_429_after_token_limit() {
        let mut limiter = ValidRequestLimiter::new(2, Duration::from_secs(60));
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 8));
        let token_id = "mcp_tok_1";

        assert_eq!(
            limiter.record_request(ip, token_id),
            ValidRequestDecision::Allowed
        );
        assert_eq!(
            limiter.record_request(ip, token_id),
            ValidRequestDecision::Allowed
        );
        assert_eq!(
            limiter.record_request(ip, token_id),
            ValidRequestDecision::RateLimited {
                status: 429,
                retry_after: Duration::from_secs(60),
            }
        );
    }

    #[test]
    fn test_valid_request_limiter_returns_429_after_ip_limit() {
        let mut limiter = ValidRequestLimiter::new(2, Duration::from_secs(60));
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));

        assert_eq!(
            limiter.record_request(ip, "token_a"),
            ValidRequestDecision::Allowed
        );
        assert_eq!(
            limiter.record_request(ip, "token_b"),
            ValidRequestDecision::Allowed
        );
        assert_eq!(
            limiter.record_request(ip, "token_c"),
            ValidRequestDecision::RateLimited {
                status: 429,
                retry_after: Duration::from_secs(60),
            }
        );
    }

    #[test]
    fn test_concurrency_limiter_rejects_when_global_cap_is_exhausted() {
        let limiter = RequestConcurrencyLimiter::new(1);
        let permit = limiter
            .try_acquire()
            .expect("first request gets the permit");

        assert_eq!(
            limiter.try_acquire().unwrap_err(),
            ConcurrencyDecision::RateLimited { status: 429 }
        );

        drop(permit);
        assert!(limiter.try_acquire().is_ok());
    }
}
