use axum::{
    extract::{Request, ConnectInfo},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;
use tracing::{debug};
use crate::settings::Settings;

pub async fn access_control(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let settings = Settings::global();
    
    // Get IP address from the connection
    let remote_ip = addr.ip().to_string();

    debug!("Request IP: {}", remote_ip);

    // Check IP access control
    if !settings.has_access(&remote_ip) {
        return Err((StatusCode::FORBIDDEN, "403 Forbidden\n").into_response());
    }

    // Check token authentication if enabled
    if settings.auth_enabled {
        let auth_header = request.headers().get(AUTHORIZATION);
        match auth_header {
            Some(header_value) => {
                let header_str = header_value.to_str().unwrap_or("");
                if !header_str.starts_with("Bearer ") {
                    debug!("Invalid authorization header format");
                    return Err((StatusCode::UNAUTHORIZED, "401 Unauthorized\n").into_response());
                }
                let token = &header_str[7..]; // Skip "Bearer "
                if let Some(expected_token) = &settings.auth_token {
                    if token != expected_token {
                        debug!("Invalid token provided");
                        return Err((StatusCode::UNAUTHORIZED, "401 Unauthorized\n").into_response());
                    }
                } else {
                    debug!("Auth enabled but no token configured");
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, "500 Internal Server Error\n").into_response());
                }
            }
            None => {
                debug!("Authorization header missing");
                return Err((StatusCode::UNAUTHORIZED, "401 Unauthorized\n").into_response());
            }
        }
    }

    Ok(next.run(request).await)
}
