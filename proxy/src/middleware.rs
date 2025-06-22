use axum::{
    extract::{Request, ConnectInfo},
    http::StatusCode,
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

    if settings.has_access(&remote_ip) {
        Ok(next.run(request).await)
    } else {
        Err((StatusCode::BAD_REQUEST, "400 Invalid Request\n").into_response())
    }
}
