use axum::{middleware, routing::get, Router};
use clap::Parser;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt};

#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(unix)]
use tower::Service;

#[cfg(unix)]
use hyper::{body::Incoming, Request};

mod api;
mod bgpmap;
mod csp;
mod handlers;
mod proxy_client;
mod settings;
mod static_files;
mod summary_parser;
mod telegram;
mod templates;
mod whois;

use settings::Settings;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Server name prefixes, separated by comma
    #[arg(long, env = "BIRDLG_SERVERS", value_delimiter = ',')]
    servers: Vec<String>,

    /// Server name domain suffixes
    #[arg(long, env = "BIRDLG_DOMAIN", default_value = "")]
    domain: String,

    /// Address bird-lg is listening on (TCP port or Unix socket path)
    #[arg(long, env = "BIRDLG_LISTEN", default_value = "5000")]
    listen: String,

    /// Port bird-lgproxy is running on
    #[arg(long, env = "BIRDLG_PROXY_PORT", default_value = "8000")]
    proxy_port: u16,

    /// Whois server for queries
    #[arg(long, env = "BIRDLG_WHOIS", default_value = "whois.verisign-grs.com")]
    whois: String,

    /// DNS zone to query ASN information
    #[arg(long, env = "BIRDLG_DNS_INTERFACE", default_value = "asn.cymru.com")]
    dns_interface: String,

    /// The infos displayed in bgpmap, separated by comma
    #[arg(
        long,
        env = "BIRDLG_BGPMAP_INFO",
        default_value = "asn,as-name,ASName,descr"
    )]
    bgpmap_info: String,

    /// Prefix of page titles in browser tabs
    #[arg(long, env = "BIRDLG_TITLE_BRAND", default_value = "Bird-lg Rust")]
    title_brand: String,

    /// Brand to show in the navigation bar
    #[arg(long, env = "BIRDLG_NAVBAR_BRAND", default_value = "Bird-lg Rust")]
    navbar_brand: String,

    /// The url of the brand to show in the navigation bar
    #[arg(long, env = "BIRDLG_NAVBAR_BRAND_URL", default_value = "/")]
    navbar_brand_url: String,

    /// The text of "All servers" button in the navigation bar
    #[arg(long, env = "BIRDLG_NAVBAR_ALL_SERVERS", default_value = "ALL Servers")]
    navbar_all_servers: String,

    /// The URL of "All servers" button
    #[arg(long, env = "BIRDLG_NAVBAR_ALL_URL", default_value = "all")]
    navbar_all_url: String,

    /// Apply network-specific changes for some networks
    #[arg(long, env = "BIRDLG_NET_SPECIFIC_MODE", default_value = "")]
    net_specific_mode: String,

    /// Protocol types to show in summary tables (comma separated list)
    #[arg(long, env = "BIRDLG_PROTOCOL_FILTER", value_delimiter = ',')]
    protocol_filter: Option<Vec<String>>,

    /// Protocol names to hide in summary tables (RE2 syntax)
    #[arg(long, env = "BIRDLG_NAME_FILTER", default_value = "")]
    name_filter: String,

    /// Time before request timed out, in seconds
    #[arg(long, env = "BIRDLG_TIMEOUT", default_value = "120")]
    timeout: u64,

    /// Telegram bot name
    #[arg(long, env = "BIRDLG_TELEGRAM_BOT_NAME", default_value = "")]
    telegram_bot_name: String,

    /// Enable token-based authentication for proxy requests
    #[arg(long, env = "BIRDLG_AUTH_ENABLED", default_value_t = false)]
    auth_enabled: bool,

    /// Authentication token for proxy requests
    #[arg(long, env = "BIRDLG_AUTH_TOKEN")]
    auth_token: Option<String>,
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
                    let hyper_service =
                        hyper::service::service_fn(move |request: Request<Incoming>| {
                            app_clone.clone().call(request)
                        });

                    if let Err(err) = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
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
        // Summary route without servers - redirect to all servers
        .route("/summary", get(handlers::redirect_to_summary))
        .route("/summary/", get(handlers::redirect_to_summary))
        // Bird protocol queries
        .route("/summary/:servers", get(handlers::bird_summary))
        .route("/summary/:servers/", get(handlers::bird_summary))
        .route("/detail/:servers/:protocol", get(handlers::bird_detail))
        .route("/detail/:servers/:protocol/", get(handlers::bird_detail))
        .route("/route/:servers/:route", get(handlers::bird_route))
        .route("/route/:servers/:route/", get(handlers::bird_route))
        .route("/route_all/:servers/:route", get(handlers::bird_route_all))
        .route("/route_all/:servers/:route/", get(handlers::bird_route_all))
        .route(
            "/route_where/:servers/:prefix",
            get(handlers::bird_route_where),
        )
        .route(
            "/route_where/:servers/:prefix/",
            get(handlers::bird_route_where),
        )
        .route(
            "/route_where_all/:servers/:prefix",
            get(handlers::bird_route_where_all),
        )
        .route(
            "/route_where_all/:servers/:prefix/",
            get(handlers::bird_route_where_all),
        )
        .route(
            "/route_bgpmap/:servers/:route",
            get(handlers::bird_route_bgpmap),
        )
        .route(
            "/route_bgpmap/:servers/:route/",
            get(handlers::bird_route_bgpmap),
        )
        .route(
            "/route_where_bgpmap/:servers/:prefix",
            get(handlers::bird_route_where_bgpmap),
        )
        .route(
            "/route_where_bgpmap/:servers/:prefix/",
            get(handlers::bird_route_where_bgpmap),
        )
        .route(
            "/route_from_protocol/:servers/:protocol",
            get(handlers::bird_route_from_protocol),
        )
        .route(
            "/route_from_protocol/:servers/:protocol/",
            get(handlers::bird_route_from_protocol),
        )
        .route(
            "/route_from_protocol_all/:servers/:protocol",
            get(handlers::bird_route_from_protocol_all),
        )
        .route(
            "/route_from_protocol_all/:servers/:protocol/",
            get(handlers::bird_route_from_protocol_all),
        )
        .route(
            "/route_from_protocol_primary/:servers/:protocol",
            get(handlers::bird_route_from_protocol_primary),
        )
        .route(
            "/route_from_protocol_primary/:servers/:protocol/",
            get(handlers::bird_route_from_protocol_primary),
        )
        .route(
            "/route_from_protocol_all_primary/:servers/:protocol",
            get(handlers::bird_route_from_protocol_all_primary),
        )
        .route(
            "/route_from_protocol_all_primary/:servers/:protocol/",
            get(handlers::bird_route_from_protocol_all_primary),
        )
        .route(
            "/route_filtered_from_protocol/:servers/:protocol",
            get(handlers::bird_route_filtered_from_protocol),
        )
        .route(
            "/route_filtered_from_protocol/:servers/:protocol/",
            get(handlers::bird_route_filtered_from_protocol),
        )
        .route(
            "/route_filtered_from_protocol_all/:servers/:protocol",
            get(handlers::bird_route_filtered_from_protocol_all),
        )
        .route(
            "/route_filtered_from_protocol_all/:servers/:protocol/",
            get(handlers::bird_route_filtered_from_protocol_all),
        )
        .route(
            "/route_from_origin/:servers/:asn",
            get(handlers::bird_route_from_origin),
        )
        .route(
            "/route_from_origin/:servers/:asn/",
            get(handlers::bird_route_from_origin),
        )
        .route(
            "/route_from_origin_all/:servers/:asn",
            get(handlers::bird_route_from_origin_all),
        )
        .route(
            "/route_from_origin_all/:servers/:asn/",
            get(handlers::bird_route_from_origin_all),
        )
        .route(
            "/route_from_origin_primary/:servers/:asn",
            get(handlers::bird_route_from_origin_primary),
        )
        .route(
            "/route_from_origin_primary/:servers/:asn/",
            get(handlers::bird_route_from_origin_primary),
        )
        .route(
            "/route_from_origin_all_primary/:servers/:asn",
            get(handlers::bird_route_from_origin_all_primary),
        )
        .route(
            "/route_from_origin_all_primary/:servers/:asn/",
            get(handlers::bird_route_from_origin_all_primary),
        )
        .route(
            "/route_generic/:servers/:command",
            get(handlers::bird_route_generic),
        )
        .route(
            "/route_generic/:servers/:command/",
            get(handlers::bird_route_generic),
        )
        .route("/generic/:servers/:command", get(handlers::bird_generic))
        .route("/generic/:servers/:command/", get(handlers::bird_generic))
        // Traceroute
        .route("/traceroute/:servers/:target", get(handlers::traceroute))
        .route("/traceroute/:servers/:target/", get(handlers::traceroute))
        // Whois
        .route("/whois/:target", get(handlers::whois))
        .route("/whois/:target/", get(handlers::whois))
        // API endpoints
        .route("/api/bird/:servers/:command", get(api::bird_api))
        .route("/api/bird/:servers/:command/", get(api::bird_api))
        .route("/api/traceroute/:servers/:target", get(api::traceroute_api))
        .route(
            "/api/traceroute/:servers/:target/",
            get(api::traceroute_api),
        )
        .route("/api/whois/:target", get(api::whois_api))
        .route("/api/whois/:target/", get(api::whois_api))
        // Telegram bot webhook (if enabled)
        .route(
            "/telegram",
            get(telegram::telegram_webhook).post(telegram::telegram_webhook),
        )
        .route(
            "/telegram/*servers",
            get(telegram::telegram_webhook).post(telegram::telegram_webhook),
        )
        // Static assets
        .route("/static/*path", get(static_files::serve_static))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(csp::csp_middleware))
                .layer(TraceLayer::new_for_http()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, ffi::OsString};

    const CONFIG_ENV_VARS: &[&str] = &[
        "BIRDLG_SERVERS",
        "BIRDLG_DOMAIN",
        "BIRDLG_LISTEN",
        "BIRDLG_PROXY_PORT",
        "BIRDLG_WHOIS",
        "BIRDLG_DNS_INTERFACE",
        "BIRDLG_BGPMAP_INFO",
        "BIRDLG_TITLE_BRAND",
        "BIRDLG_NAVBAR_BRAND",
        "BIRDLG_NAVBAR_BRAND_URL",
        "BIRDLG_NAVBAR_ALL_SERVERS",
        "BIRDLG_NAVBAR_ALL_URL",
        "BIRDLG_NET_SPECIFIC_MODE",
        "BIRDLG_PROTOCOL_FILTER",
        "BIRDLG_NAME_FILTER",
        "BIRDLG_TIMEOUT",
        "BIRDLG_TELEGRAM_BOT_NAME",
        "BIRDLG_AUTH_ENABLED",
        "BIRDLG_AUTH_TOKEN",
    ];

    struct EnvGuard(Vec<(&'static str, Option<OsString>)>);

    impl EnvGuard {
        fn new() -> Self {
            Self(
                CONFIG_ENV_VARS
                    .iter()
                    .map(|key| (*key, env::var_os(key)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn environment_configuration_has_expected_precedence_and_validation() {
        let _guard = EnvGuard::new();
        let values = [
            ("BIRDLG_SERVERS", "edge-a,192.0.2.10"),
            ("BIRDLG_DOMAIN", "example.net"),
            ("BIRDLG_LISTEN", "5100"),
            ("BIRDLG_PROXY_PORT", "8100"),
            ("BIRDLG_WHOIS", "whois.example.net"),
            ("BIRDLG_DNS_INTERFACE", "asn.example.net"),
            ("BIRDLG_BGPMAP_INFO", "asn,descr"),
            ("BIRDLG_TITLE_BRAND", "Example LG"),
            ("BIRDLG_NAVBAR_BRAND", "Example Network"),
            ("BIRDLG_NAVBAR_BRAND_URL", "https://example.net"),
            ("BIRDLG_NAVBAR_ALL_SERVERS", "All PoPs"),
            ("BIRDLG_NAVBAR_ALL_URL", "all-pops"),
            ("BIRDLG_NET_SPECIFIC_MODE", "dn42"),
            ("BIRDLG_PROTOCOL_FILTER", "BGP,Pipe"),
            ("BIRDLG_NAME_FILTER", "internal.*"),
            ("BIRDLG_TIMEOUT", "45"),
            ("BIRDLG_TELEGRAM_BOT_NAME", "example_bot"),
            ("BIRDLG_AUTH_ENABLED", "true"),
            ("BIRDLG_AUTH_TOKEN", "environment-token"),
        ];
        for (key, value) in values {
            env::set_var(key, value);
        }

        let args = Args::try_parse_from(["bird-lg-rs"]).unwrap();
        assert_eq!(args.servers, ["edge-a", "192.0.2.10"]);
        assert_eq!(args.domain, "example.net");
        assert_eq!(args.listen, "5100");
        assert_eq!(args.proxy_port, 8100);
        assert_eq!(args.whois, "whois.example.net");
        assert_eq!(args.dns_interface, "asn.example.net");
        assert_eq!(args.bgpmap_info, "asn,descr");
        assert_eq!(args.title_brand, "Example LG");
        assert_eq!(args.navbar_brand, "Example Network");
        assert_eq!(args.navbar_brand_url, "https://example.net");
        assert_eq!(args.navbar_all_servers, "All PoPs");
        assert_eq!(args.navbar_all_url, "all-pops");
        assert_eq!(args.net_specific_mode, "dn42");
        assert_eq!(args.protocol_filter.unwrap(), ["BGP", "Pipe"]);
        assert_eq!(args.name_filter, "internal.*");
        assert_eq!(args.timeout, 45);
        assert_eq!(args.telegram_bot_name, "example_bot");
        assert!(args.auth_enabled);
        assert_eq!(args.auth_token.as_deref(), Some("environment-token"));

        let settings = Settings::from_args(Args::try_parse_from(["bird-lg-rs"]).unwrap()).unwrap();
        assert_eq!(settings.servers, ["edge-a.example.net", "192.0.2.10"]);
        let settings_debug = format!("{:?}", settings);
        assert!(!settings_debug.contains("environment-token"));
        assert!(settings_debug.contains("auth_token: Some(\"[REDACTED]\")"));

        env::set_var("BIRDLG_AUTH_ENABLED", "false");
        let cli_args = Args::try_parse_from([
            "bird-lg-rs",
            "--servers=cli-a,cli-b",
            "--proxy-port=9100",
            "--auth-enabled",
        ])
        .unwrap();
        assert_eq!(cli_args.servers, ["cli-a", "cli-b"]);
        assert_eq!(cli_args.proxy_port, 9100);
        assert!(cli_args.auth_enabled);

        let mut invalid = Args::try_parse_from(["bird-lg-rs"]).unwrap();
        invalid.servers.clear();
        assert_eq!(
            Settings::from_args(invalid).unwrap_err().to_string(),
            "At least one non-empty server must be configured"
        );

        for invalid_token in [None, Some(String::new()), Some(" ".to_string())] {
            let mut invalid = Args::try_parse_from(["bird-lg-rs"]).unwrap();
            invalid.auth_enabled = true;
            invalid.auth_token = invalid_token;
            assert_eq!(
                Settings::from_args(invalid).unwrap_err().to_string(),
                "Authentication token is required when authentication is enabled"
            );
        }
    }
}
