use axum::{
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
    extract::Request,
};

const CSP_HEADER_VALUE: &str = "default-src 'self'; \
    script-src 'self'; \
    style-src 'self'; \
    img-src 'self' data:; \
    frame-ancestors 'none'; \
    base-uri 'self'; \
    form-action 'self'";

pub async fn csp_middleware(
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP_HEADER_VALUE),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_disallows_inline_scripts_and_styles() {
        assert!(!CSP_HEADER_VALUE.contains("'unsafe-inline'"));
        assert!(CSP_HEADER_VALUE.contains("script-src 'self'"));
        assert!(CSP_HEADER_VALUE.contains("style-src 'self'"));
    }
}
