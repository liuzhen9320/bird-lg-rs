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
    let remote_ip = {
        let x_forwarded_for = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .map(|s| {
                s.split(',').next().unwrap_or("").trim().to_string()
            })
            .filter(|s| !s.is_empty());
        let x_real_ip = x_forwarded_for
            .or_else(|| {
                request
                    .headers()
                    .get("x-real-ip")
                    .and_then(|h| h.to_str().ok())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            });
        let fallback_ip = x_real_ip
            .or_else(|| {
                request
                    .remote_addr()
                    .map(|addr| addr.ip().to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        fallback_ip
    };

    if settings.has_access(remote_ip) {
        Ok(next.run(request).await)
    } else {
        Err((StatusCode::BAD_REQUEST, "Invalid Request\n").into_response())
    }
} 
