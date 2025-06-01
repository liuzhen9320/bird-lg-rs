use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use crate::settings::Settings;

pub async fn access_control(request: Request, next: Next) -> Result<Response, Response> {
    let settings = Settings::global();
    
    // Get remote address from request
    let remote_addr = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
        })
        .unwrap_or("unknown");

    if settings.has_access(remote_addr) {
        Ok(next.run(request).await)
    } else {
        Err((StatusCode::INTERNAL_SERVER_ERROR, "Invalid Request\n").into_response())
    }
} 