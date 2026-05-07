use axum::{
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
    extract::Request,
};

const CSP_HEADER_VALUE: &str = "default-src 'self'; \
    script-src 'self' 'unsafe-inline'; \
    style-src 'self' 'unsafe-inline'; \
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
