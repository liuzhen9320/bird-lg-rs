use crate::settings::Settings;
use axum::{
    extract::{ConnectInfo, Request},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use tracing::debug;

fn unauthorized_response() -> Response {
    (StatusCode::UNAUTHORIZED, "401 Unauthorized\n").into_response()
}

fn constant_time_token_eq(provided: &str, expected: &str) -> bool {
    bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
}

fn authorize(headers: &HeaderMap, expected_token: Option<&str>) -> Result<(), StatusCode> {
    let provided_token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    match (provided_token, expected_token) {
        (Some(provided), Some(expected)) if constant_time_token_eq(provided, expected) => Ok(()),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

pub async fn access_control(request: Request, next: Next) -> Result<Response, Response> {
    let settings = Settings::global();

    // Check IP access control (TCP connections only; Unix sockets are local)
    if let Some(ConnectInfo(addr)) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        let remote_ip = addr.ip();
        debug!("Request IP: {}", remote_ip);
        if !settings.has_access(remote_ip) {
            return Err((StatusCode::FORBIDDEN, "403 Forbidden\n").into_response());
        }
    }

    // Check token authentication if enabled
    if settings.auth_enabled
        && authorize(request.headers(), settings.auth_token.as_deref()).is_err()
    {
        debug!("Authentication failed");
        return Err(unauthorized_response());
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn token_comparison_accepts_only_an_exact_match() {
        assert!(constant_time_token_eq("secret-token", "secret-token"));
        assert!(!constant_time_token_eq("secret-tokee", "secret-token"));
        assert!(!constant_time_token_eq("short", "secret-token"));
    }

    #[test]
    fn authentication_failures_always_return_unauthorized() {
        let cases = [
            (None, Some("secret-token")),
            (Some("Basic secret-token"), Some("secret-token")),
            (Some("Bearer wrong-token"), Some("secret-token")),
            (Some("Bearer "), Some("secret-token")),
            (Some("Bearer secret-token"), None),
        ];

        for (header, expected_token) in cases {
            let mut headers = HeaderMap::new();
            if let Some(header) = header {
                headers.insert(AUTHORIZATION, HeaderValue::from_static(header));
            }

            assert_eq!(
                authorize(&headers, expected_token),
                Err(StatusCode::UNAUTHORIZED)
            );
        }
    }

    #[test]
    fn valid_bearer_token_is_authorized() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer secret-token"),
        );

        assert!(authorize(&headers, Some("secret-token")).is_ok());
    }
}
