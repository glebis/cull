use cull_lib::preview::web_stream::{
    constant_time_token_matches, preview_web_stream_bind_host, preview_web_stream_port_candidates,
    preview_web_stream_public_host, preview_web_stream_url, token_from_query,
    PreviewWebStreamToken,
};

#[test]
fn preview_web_stream_url_contains_session_token() {
    let token = PreviewWebStreamToken::for_test("token-123");

    let url = preview_web_stream_url("127.0.0.1", 8723, &token);

    assert_eq!(url, "http://127.0.0.1:8723/?token=token-123");
}

#[test]
fn preview_web_stream_defaults_to_loopback_and_requires_explicit_lan_bind() {
    assert_eq!(preview_web_stream_bind_host(None), "127.0.0.1");
    assert_eq!(preview_web_stream_bind_host(Some("")), "127.0.0.1");
    assert_eq!(preview_web_stream_bind_host(Some("localhost")), "127.0.0.1");
    assert_eq!(preview_web_stream_bind_host(Some("0.0.0.0")), "0.0.0.0");
    assert_eq!(
        preview_web_stream_bind_host(Some("192.168.0.4")),
        "127.0.0.1"
    );
    assert_eq!(preview_web_stream_public_host("127.0.0.1"), "127.0.0.1");
}

#[test]
fn preview_web_stream_port_candidates_try_requested_then_fallbacks() {
    let candidates = preview_web_stream_port_candidates(8723);

    assert_eq!(candidates[0], 8723);
    assert!(candidates.contains(&8724));
    assert_eq!(candidates.last().copied(), Some(0));
}

#[test]
fn preview_web_stream_token_is_extracted_from_query() {
    assert_eq!(
        token_from_query("token=abc123&ignored=true").as_deref(),
        Some("abc123")
    );
    assert_eq!(token_from_query("ignored=true"), None);
}

#[test]
fn preview_web_stream_token_match_rejects_missing_or_wrong_token() {
    let expected = PreviewWebStreamToken::for_test("token-123");

    assert!(constant_time_token_matches(&expected, Some("token-123")));
    assert!(!constant_time_token_matches(&expected, Some("token-124")));
    assert!(!constant_time_token_matches(&expected, None));
}
