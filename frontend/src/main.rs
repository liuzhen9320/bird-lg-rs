use axum::{
    routing::get,
    Router,
};
use clap::Parser;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::{
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;
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
mod handlers;
mod templates;
mod proxy_client;
mod bgpmap;
mod whois;
mod api;
mod telegram;

use settings::Settings;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Server name prefixes, separated by comma
    #[arg(long, value_delimiter = ',')]
    servers: Vec<String>,

    /// Server name domain suffixes
    #[arg(long, default_value = "")]
    domain: String,

    /// Address bird-lg is listening on (TCP port or Unix socket path)
    #[arg(long, default_value = "5000")]
    listen: String,

    /// Port bird-lgproxy is running on
    #[arg(long, default_value = "8000")]
    proxy_port: u16,

    /// Whois server for queries
    #[arg(long, default_value = "whois.verisign-grs.com")]
    whois: String,

    /// DNS zone to query ASN information
    #[arg(long, default_value = "asn.cymru.com")]
    dns_interface: String,

    /// The infos displayed in bgpmap, separated by comma
    #[arg(long, default_value = "asn,as-name,ASName,descr")]
    bgpmap_info: String,

    /// Prefix of page titles in browser tabs
    #[arg(long, default_value = "Bird-lg Rust")]
    title_brand: String,

    /// Brand to show in the navigation bar
    #[arg(long, default_value = "Bird-lg Rust")]
    navbar_brand: String,

    /// The url of the brand to show in the navigation bar
    #[arg(long, default_value = "/")]
    navbar_brand_url: String,

    /// The text of "All servers" button in the navigation bar
    #[arg(long, default_value = "ALL Servers")]
    navbar_all_servers: String,

    /// The URL of "All servers" button
    #[arg(long, default_value = "all")]
    navbar_all_url: String,

    /// Apply network-specific changes for some networks
    #[arg(long, default_value = "")]
    net_specific_mode: String,

    /// Protocol types to show in summary tables (comma separated list)
    #[arg(long, value_delimiter = ',')]
    protocol_filter: Option<Vec<String>>,

    /// Protocol names to hide in summary tables (RE2 syntax)
    #[arg(long, default_value = "")]
    name_filter: String,

    /// Time before request timed out, in seconds
    #[arg(long, default_value = "120")]
    timeout: u64,

    /// Telegram bot name
    #[arg(long, default_value = "")]
    telegram_bot_name: String,
}

/// Create Unix socket listener on Unix systems
#[cfg(unix)]
async fn create_unix_listener(socket_path: &str) -> anyhow::Result<()> {
    // Delete existing socket file, ignore errors
    let _ = fs::remove_file(socket_path);
    
    let listener = UnixListener::bind(socket_path)?;
    
    // Set socket permissions to 666 (readable and writable by all)
    if let Err(e) = fs::set_permissions(socket_path, fs::Permissions::from_mode(0o666)) {
        tracing::warn!("Failed to set socket permissions: {}", e);
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
                        tracing::warn!("Error serving connection: {:?}", err);
                    }
                });
            }
            Err(e) => {
                tracing::warn!("Failed to accept Unix socket connection: {}", e);
            }
        }
    }
}

/// Build the application router
async fn build_router() -> Router {
    Router::new()
        // Main page redirects to all servers summary
        .route("/", get(handlers::redirect_to_summary))
        
        // Bird protocol queries
        .route("/summary/:servers", get(handlers::bird_summary))
        .route("/detail/:servers/:protocol", get(handlers::bird_detail))
        .route("/route/:servers/:route", get(handlers::bird_route))
        .route("/route_all/:servers/:route", get(handlers::bird_route_all))
        .route("/route_where/:servers/:prefix", get(handlers::bird_route_where))
        .route("/route_where_all/:servers/:prefix", get(handlers::bird_route_where_all))
        .route("/route_bgpmap/:servers/:route", get(handlers::bird_route_bgpmap))
        .route("/route_where_bgpmap/:servers/:prefix", get(handlers::bird_route_where_bgpmap))
        .route("/route_from_protocol/:servers/:protocol", get(handlers::bird_route_from_protocol))
        .route("/route_from_protocol_all/:servers/:protocol", get(handlers::bird_route_from_protocol_all))
        .route("/route_filtered_from_protocol/:servers/:protocol", get(handlers::bird_route_filtered_from_protocol))
        .route("/route_filtered_from_protocol_all/:servers/:protocol", get(handlers::bird_route_filtered_from_protocol_all))
        .route("/route_from_origin/:servers/:asn", get(handlers::bird_route_from_origin))
        .route("/route_from_origin_all/:servers/:asn", get(handlers::bird_route_from_origin_all))
        .route("/generic/:servers/:command", get(handlers::bird_generic))
        
        // Traceroute
        .route("/traceroute/:servers/:target", get(handlers::traceroute))
        
        // Whois
        .route("/whois/:target", get(handlers::whois))
        
        // API endpoints
        .route("/api/bird/:servers/:command", get(api::bird_api))
        .route("/api/traceroute/:servers/:target", get(api::traceroute_api))
        .route("/api/whois/:target", get(api::whois_api))
        
        // Telegram bot webhook (if enabled)
        .route("/telegram", get(telegram::telegram_webhook).post(telegram::telegram_webhook))
        
        // Static assets
        .nest_service("/static", ServeDir::new("assets/static"))
        
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
        )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bird_lg_rs=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    
    // Initialize settings
    Settings::init(args).await?;

    // Initialize templates
    templates::init()?;

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
    
    let app = build_router().await;
    axum::serve(listener, app).await?;

    Ok(())
} 