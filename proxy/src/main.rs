use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use serde::Deserialize;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
};

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(unix)]
use tower::Service;

#[cfg(unix)]
use hyper::{body::Incoming, Request};

mod settings;
mod bird;
mod traceroute;
mod middleware;

use settings::Settings;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// IPs or networks allowed to access this proxy, separated by commas
    #[arg(long, value_delimiter = ',')]
    allowed: Option<Vec<String>>,

    /// Socket file for bird
    #[arg(long, default_value = "/var/run/bird/bird.ctl")]
    bird: String,

    /// Listen address (TCP port or Unix socket path)
    #[arg(long, default_value = "8000")]
    listen: String,

    /// Traceroute binary file
    #[arg(long)]
    traceroute_bin: Option<String>,

    /// Traceroute flags, supports multiple flags separated with space
    #[arg(long)]
    traceroute_flags: Option<String>,

    /// Whether to display traceroute outputs raw
    #[arg(long)]
    traceroute_raw: bool,
}

#[derive(Deserialize)]
struct BirdQuery {
    q: String,
}

#[derive(Deserialize)]
struct TracerouteQuery {
    q: String,
}

// Default handler, returns 500 Internal Server Error
async fn invalid_handler() -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Invalid Request\n")
}

// Handles BIRD queries
async fn bird_handler(
    Query(params): Query<BirdQuery>,
) -> Result<impl IntoResponse, Response> {
    if params.q.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Query parameter 'q' is required").into_response());
    }

    match bird::execute_bird_command(&params.q).await {
        Ok(output) => Ok(output),
        Err(e) => {
            warn!("Bird command failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
        }
    }
}

// Handles traceroute queries
async fn traceroute_handler(
    Query(params): Query<TracerouteQuery>,
) -> Result<impl IntoResponse, Response> {
    if params.q.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Query parameter 'q' is required").into_response());
    }

    match traceroute::execute_traceroute(&params.q).await {
        Ok(output) => Ok(output),
        Err(e) => {
            warn!("Traceroute command failed: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
        }
    }
}

/// Create Unix socket listener on Unix systems
#[cfg(unix)]
async fn create_unix_listener(socket_path: &str) -> anyhow::Result<()> {
    // Delete existing socket file, ignore errors
    let _ = fs::remove_file(socket_path);
    
    let listener = UnixListener::bind(socket_path)?;
    
    // Set socket permissions to 666 (readable and writable by all)
    if let Err(e) = fs::set_permissions(socket_path, fs::Permissions::from_mode(0o666)) {
        warn!("Failed to set socket permissions: {}", e);
    }
    
    info!("Server started on Unix socket: {}", socket_path);
    
    let app = build_router().await;
    
    // Manually handle Unix socket connections
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let app_clone = app.clone();
                tokio::spawn(async move {
                    let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                        app_clone.clone().call(request)
                    });
                    
                    if let Err(err) = hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                        .serve_connection(hyper_util::rt::TokioIo::new(stream), hyper_service)
                        .await
                    {
                        warn!("Error serving connection: {:?}", err);
                    }
                });
            }
            Err(e) => {
                warn!("Failed to accept Unix socket connection: {}", e);
            }
        }
    }
}

/// Build the application router
async fn build_router() -> Router {
    Router::new()
        .route("/", get(invalid_handler))
        .route("/bird", get(bird_handler))
        .route("/bird6", get(bird_handler))
        .route("/traceroute", get(traceroute_handler))
        .route("/traceroute6", get(traceroute_handler))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(middleware::access_control))
        )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bird_lgproxy_rs=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    
    // Initialize settings
    Settings::init(args).await?;

    // Initialize traceroute
    traceroute::init().await;

    let settings = Settings::global();
    info!("Listening on {}...", settings.listen);

    // Determine listen address type and start server
    #[cfg(unix)]
    if settings.listen.starts_with('/') {
        // Unix socket on Unix systems
        return create_unix_listener(&settings.listen).await;
    }
    
    // TCP socket (default for non-Unix systems or TCP addresses on Unix)
    let addr = if settings.listen.contains(':') {
        settings.listen.parse::<SocketAddr>()?
    } else {
        format!("0.0.0.0:{}", settings.listen).parse::<SocketAddr>()?
    };
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server started on TCP: {}", addr);
    
    let app = build_router().await
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await?;

    Ok(())
} 
