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
use std::{fs, os::unix::fs::PermissionsExt};

#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(unix)]
use tower::Service;

#[cfg(unix)]
use hyper::{body::Incoming, Request};

mod bird;
mod middleware;
mod settings;
mod traceroute;

use settings::Settings;

const BIRD_QUERY_REQUIRED_MESSAGE: &str = "Query parameter 'q' is required";
const BIRD_QUERY_NOT_ALLOWED_MESSAGE: &str =
    "Query not allowed. Only 'show protocols' and 'show route' commands are permitted.";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// IPs or networks allowed to access this proxy, separated by commas
    #[arg(long, env = "ALLOWED_IPS", value_delimiter = ',')]
    allowed: Option<Vec<String>>,

    /// Socket file for bird
    #[arg(long, env = "BIRD_SOCKET", default_value = "/var/run/bird/bird.ctl")]
    bird: String,

    /// Listen address (TCP port or Unix socket path)
    #[arg(long, env = "BIRDLG_PROXY_PORT", default_value = "8000")]
    listen: String,

    /// Traceroute binary file
    #[arg(long, env = "BIRDLG_TRACEROUTE_BIN")]
    traceroute_bin: Option<String>,

    /// Traceroute flags, supports multiple flags separated with space
    #[arg(long, env = "BIRDLG_TRACEROUTE_FLAGS")]
    traceroute_flags: Option<String>,

    /// Whether to display traceroute outputs raw
    #[arg(long, env = "BIRDLG_TRACEROUTE_RAW")]
    traceroute_raw: bool,

    /// Maximum number of concurrent traceroute requests
    #[arg(long, env = "BIRDLG_TRACEROUTE_MAX_CONCURRENT", default_value_t = 10)]
    traceroute_max_concurrent: usize,

    /// Restrict Bird queries to show protocols and show route commands
    #[arg(long, env = "BIRDLG_BIRD_RESTRICT_CMDS", default_value_t = true)]
    bird_restrict_cmds: bool,

    /// Enable token-based authentication
    #[arg(long, env = "BIRDLG_AUTH_ENABLED", default_value_t = false)]
    auth_enabled: bool,

    /// Authentication token for API access
    #[arg(long, env = "BIRDLG_AUTH_TOKEN")]
    auth_token: Option<String>,
}

#[derive(Deserialize)]
struct BirdQuery {
    q: String,
}

#[derive(Deserialize)]
struct TracerouteQuery {
    q: String,
}

fn is_bird_command_allowed(query: &str) -> bool {
    let query_lower = query.to_lowercase();
    query_lower.starts_with("show protocols") || query_lower.starts_with("show route")
}

fn strip_control_chars(query: &str) -> String {
    query.chars().filter(|c| !c.is_control()).collect()
}

fn prepare_bird_query(
    query: &str,
    restrict_cmds: bool,
) -> Result<String, (StatusCode, &'static str)> {
    if query.is_empty() {
        return Err((StatusCode::BAD_REQUEST, BIRD_QUERY_REQUIRED_MESSAGE));
    }

    // Drop control characters from the query before restriction checks and execution.
    let query = strip_control_chars(query);

    if restrict_cmds && !is_bird_command_allowed(&query) {
        return Err((StatusCode::BAD_REQUEST, BIRD_QUERY_NOT_ALLOWED_MESSAGE));
    }

    Ok(query)
}

// Default handler, returns project info
async fn index_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        "bird-lg-rs\n\nhttps://github.com/liuzhen9320/bird-lg-rs\n",
    )
}

// Invalid request handler for unmatched routes
async fn invalid_handler() -> impl IntoResponse {
    (StatusCode::BAD_REQUEST, "400 Bad Request\n")
}

// Handles BIRD queries
async fn bird_handler(Query(params): Query<BirdQuery>) -> Result<impl IntoResponse, Response> {
    let settings = Settings::global();
    let query = prepare_bird_query(&params.q, settings.bird_restrict_cmds)
        .map_err(|(status, message)| (status, message).into_response())?;

    match bird::execute_bird_command(&query).await {
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
        .route("/", get(index_handler))
        .route("/bird", get(bird_handler))
        .route("/bird6", get(bird_handler))
        .route("/traceroute", get(traceroute_handler))
        .route("/traceroute6", get(traceroute_handler))
        .fallback(invalid_handler)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(middleware::access_control)),
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

    let app = build_router()
        .await
        .into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, ffi::OsString};

    const CONFIG_ENV_VARS: &[&str] = &[
        "ALLOWED_IPS", "BIRD_SOCKET", "BIRDLG_PROXY_PORT", "BIRDLG_TRACEROUTE_BIN",
        "BIRDLG_TRACEROUTE_FLAGS", "BIRDLG_TRACEROUTE_RAW",
        "BIRDLG_TRACEROUTE_MAX_CONCURRENT", "BIRDLG_BIRD_RESTRICT_CMDS",
        "BIRDLG_AUTH_ENABLED", "BIRDLG_AUTH_TOKEN",
    ];

    struct EnvGuard(Vec<(&'static str, Option<OsString>)>);

    impl EnvGuard {
        fn new() -> Self {
            Self(CONFIG_ENV_VARS.iter().map(|key| (*key, env::var_os(key))).collect())
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
            ("ALLOWED_IPS", "192.0.2.1,2001:db8::/32"),
            ("BIRD_SOCKET", "/run/bird/example.ctl"),
            ("BIRDLG_PROXY_PORT", "8100"),
            ("BIRDLG_TRACEROUTE_BIN", "/usr/bin/traceroute"),
            ("BIRDLG_TRACEROUTE_FLAGS", "-m 12 --wait=2"),
            ("BIRDLG_TRACEROUTE_RAW", "true"),
            ("BIRDLG_TRACEROUTE_MAX_CONCURRENT", "17"),
            ("BIRDLG_BIRD_RESTRICT_CMDS", "false"),
            ("BIRDLG_AUTH_ENABLED", "true"),
            ("BIRDLG_AUTH_TOKEN", "environment-token"),
        ];
        for (key, value) in values {
            env::set_var(key, value);
        }

        let args = Args::try_parse_from(["bird-lgproxy-rs"]).unwrap();
        assert_eq!(args.allowed.as_ref().unwrap(), &["192.0.2.1", "2001:db8::/32"]);
        assert_eq!(args.bird, "/run/bird/example.ctl");
        assert_eq!(args.listen, "8100");
        assert_eq!(args.traceroute_bin.as_deref(), Some("/usr/bin/traceroute"));
        assert_eq!(args.traceroute_flags.as_deref(), Some("-m 12 --wait=2"));
        assert!(args.traceroute_raw);
        assert_eq!(args.traceroute_max_concurrent, 17);
        assert!(!args.bird_restrict_cmds);
        assert!(args.auth_enabled);
        assert_eq!(args.auth_token.as_deref(), Some("environment-token"));

        let settings = Settings::from_args(
            Args::try_parse_from(["bird-lgproxy-rs"]).unwrap()
        ).unwrap();
        let allowed_nets: Vec<String> = settings.allowed_nets.iter()
            .map(ToString::to_string).collect();
        assert_eq!(allowed_nets, ["192.0.2.1/32", "2001:db8::/32"]);
        assert_eq!(settings.traceroute_flags, ["-m", "12", "--wait=2"]);

        env::set_var("BIRDLG_AUTH_ENABLED", "false");
        let cli_args = Args::try_parse_from([
            "bird-lgproxy-rs", "--allowed=198.51.100.0/24", "--listen=9100",
            "--bird-restrict-cmds", "--auth-enabled",
        ]).unwrap();
        assert_eq!(cli_args.allowed.unwrap(), ["198.51.100.0/24"]);
        assert_eq!(cli_args.listen, "9100");
        assert!(cli_args.bird_restrict_cmds);
        assert!(cli_args.auth_enabled);

        let mut invalid = Args::try_parse_from(["bird-lgproxy-rs"]).unwrap();
        invalid.auth_enabled = true;
        invalid.auth_token = Some(" ".to_string());
        assert_eq!(Settings::from_args(invalid).unwrap_err().to_string(),
            "Authentication token is required when authentication is enabled");
    }

    #[test]
    fn strip_control_chars_removes_unicode_control_characters() {
        let cases = [
            (
                "no control chars",
                "show route for 1.2.3.4",
                "show route for 1.2.3.4",
            ),
            (
                "newline without space",
                "show route\nshow memory",
                "show routeshow memory",
            ),
            (
                "newline with space",
                "show route \nshow memory",
                "show route show memory",
            ),
            (
                "carriage return without space",
                "show route\rshow memory",
                "show routeshow memory",
            ),
            (
                "carriage return with space",
                "show route \rshow memory",
                "show route show memory",
            ),
            (
                "tab without space",
                "show route\tshow memory",
                "show routeshow memory",
            ),
            (
                "tab with space",
                "show route \tshow memory",
                "show route show memory",
            ),
            ("null byte", "show\0route", "showroute"),
            ("bel and del", "show\u{0007}route\u{007f}", "showroute"),
            ("multiple control chars", "a\nb\rc\td\0e", "abcde"),
            ("only control chars", "\n\r\t\0", ""),
            ("empty string", "", ""),
        ];

        for (name, input, expected) in cases {
            assert_eq!(strip_control_chars(input), expected, "{name}");
        }
    }

    #[test]
    fn prepare_bird_query_strips_control_chars_before_restricted_check() {
        let query = prepare_bird_query("show route \nshow memory", true).unwrap();
        assert_eq!(query, "show route show memory");
    }

    #[test]
    fn prepare_bird_query_strips_control_chars_when_unrestricted() {
        let query = prepare_bird_query("show route \nshow memory", false).unwrap();
        assert_eq!(query, "show route show memory");
    }

    #[test]
    fn prepare_bird_query_rejects_forbidden_restricted_command() {
        let err = prepare_bird_query("configure", true).unwrap_err();
        assert_eq!(
            err,
            (StatusCode::BAD_REQUEST, BIRD_QUERY_NOT_ALLOWED_MESSAGE)
        );
    }
}
