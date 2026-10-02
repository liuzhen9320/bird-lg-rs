use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Router,
};
use std::{
    collections::HashMap,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

type Commands = Arc<Mutex<Vec<String>>>;

async fn bird(
    State(commands): State<Commands>,
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, &'static str) {
    let command = params.get("q").expect("missing command");
    commands.lock().unwrap().push(command.clone());
    if command.contains("fail") {
        (StatusCode::BAD_GATEWAY, "backend unavailable")
    } else if command == "show protocols" {
        (StatusCode::OK, include_str!("fixtures/summary.txt"))
    } else {
        (StatusCode::OK, include_str!("fixtures/bgpmap.txt"))
    }
}

struct Frontend {
    child: Child,
    base: String,
}

impl Drop for Frontend {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Frontend {
    async fn start(proxy_port: u16, whois_port: u16) -> Self {
        let reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let child = Command::new(env!("CARGO_BIN_EXE_bird-lg-rs"))
            .env_clear()
            .env("RUST_LOG", "off")
            .args([
                "--servers=上海<localhost>,Core<127.0.0.1>",
                &format!("--listen=127.0.0.1:{port}"),
                &format!("--proxy-port={proxy_port}"),
                &format!("--whois=127.0.0.1:{whois_port}"),
                "--timeout=2",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut frontend = Self {
            child,
            base: format!("http://127.0.0.1:{port}"),
        };
        for _ in 0..200 {
            assert!(
                frontend.child.try_wait().unwrap().is_none(),
                "frontend exited before readiness"
            );
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                return frontend;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("frontend did not start");
    }
}

async fn html(client: &reqwest::Client, base: &str, path: &str) -> String {
    let response = client.get(format!("{base}{path}")).send().await.unwrap();
    assert_eq!(response.status(), 200, "GET {path}");
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let policy = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(policy.contains("script-src 'self'"));
    assert!(!policy.contains("unsafe-inline"));
    html_escape::decode_html_entities(&response.text().await.unwrap()).into_owned()
}

#[tokio::test]
async fn pages_work_with_mock_backends_and_legacy_urls() {
    let commands: Commands = Arc::default();
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_port = proxy.local_addr().unwrap().port();
    let mock_app = Router::new()
        .route("/bird", get(bird))
        .route(
            "/traceroute",
            get(|| async { "1 192.0.2.1\n2 2001:db8::1\n" }),
        )
        .with_state(commands.clone());
    let proxy_task = tokio::spawn(async move {
        axum::serve(proxy, mock_app).await.unwrap();
    });

    let whois = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let whois_port = whois.local_addr().unwrap().port();
    let whois_task = tokio::spawn(async move {
        loop {
            let (stream, _) = whois.accept().await.unwrap();
            tokio::spawn(async move {
                let mut stream = BufReader::new(stream);
                let mut query = String::new();
                stream.read_line(&mut query).await.unwrap();
                stream
                    .get_mut()
                    .write_all(b"aut-num: AS64500\nremarks: example.net\naddress: 192.0.2.1\n")
                    .await
                    .unwrap();
            });
        }
    });

    let frontend = Frontend::start(proxy_port, whois_port).await;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let base = &frontend.base;

    let root = client.get(format!("{base}/")).send().await.unwrap();
    assert!(root.status().is_redirection());
    assert_eq!(root.headers()["location"], "/summary/localhost+127.0.0.1/");

    for path in [
        "/summary/localhost",
        "/summary/localhost/",
        "/summary/%E4%B8%8A%E6%B5%B7/",
        "/generic/localhost/protocols",
    ] {
        let result = html(&client, base, path).await;
        assert!(result.contains("<h2>上海: show protocols</h2>"));
        assert!(result.contains("href=\"/detail/localhost/int_babel\""));
        assert!(result.contains("<table "));
        assert!(result.contains("value=\"localhost\""));
    }
    let summary = html(&client, base, "/summary/localhost+127.0.0.1/").await;
    assert_eq!(summary.matches("<table ").count(), 2);
    assert!(summary
        .contains("class=\"nav-link active\"\n\t\t\t\t\thref=\"/summary/localhost+127.0.0.1/\""));

    for path in [
        "/route/localhost/192.0.2.0/24",
        "/route/localhost/192.0.2.0%2F24",
        "/route/localhost/192.0.2.0/24/",
        "/route/localhost/192.0.2.0%2F24/",
    ] {
        let result = html(&client, base, path).await;
        assert!(result.contains("value=\"192.0.2.0/24\""));
        assert!(result.contains("href=\"/route/127.0.0.1/192.0.2.0%2F24\""));
        assert_eq!(
            commands.lock().unwrap().last().unwrap(),
            "show route for 192.0.2.0/24"
        );
    }
    for path in ["/route/localhost", "/route/localhost/"] {
        let result = html(&client, base, path).await;
        assert!(result.contains("value=\"\""));
        assert_eq!(commands.lock().unwrap().last().unwrap(), "show route for");
    }
    html(&client, base, "/route/localhost/2001%3Adb8%3A%3A%2F32").await;
    assert_eq!(
        commands.lock().unwrap().last().unwrap(),
        "show route for 2001:db8::/32"
    );
    html(
        &client,
        base,
        "/generic/localhost/route%20where%20net%20%3D%20%25%20%2B%20%23%20%3F%20%26%2F",
    )
    .await;
    assert_eq!(
        commands.lock().unwrap().last().unwrap(),
        "show route where net = % + # ? &/"
    );

    for (path, expected) in [
        (
            "/detail/localhost/peer-transit",
            "show protocols all 'peer-transit'",
        ),
        (
            "/route_from_protocol_primary/localhost/peer-transit",
            "show route protocol 'peer-transit' primary",
        ),
        (
            "/route_from_origin_primary/localhost/64500",
            "show route where bgp_path.last = 64500 primary",
        ),
    ] {
        html(&client, base, path).await;
        assert_eq!(commands.lock().unwrap().last().unwrap(), expected);
    }

    for (action, command) in [
        ("route_bgpmap", "show route for 1.1.1.1 all"),
        (
            "route_where_bgpmap",
            "show route where net ~ [ 1.1.1.1 ] all",
        ),
    ] {
        let result = html(&client, base, &format!("/{action}/localhost/1.1.1.1")).await;
        assert!(result.contains(&format!("<h2>BGPmap: {command}</h2>")));
        assert!(result.contains(&format!("value=\"{action}\" selected>")));
        assert!(result.contains("data-graph=\""));
        assert!(result.contains("src=\"/static/bgpmap.js\""));
        assert!(!result.contains("<script>"));
    }

    let traceroute = html(&client, base, "/traceroute/localhost/1.1.1.1").await;
    assert!(traceroute.contains("href=\"/whois/192.0.2.1\" class=\"whois\""));
    assert!(traceroute.contains("<h2>上海: 1.1.1.1</h2>"));
    let whois = html(&client, base, "/whois/AS64500").await;
    assert!(whois.contains("href=\"/whois/example.net\" class=\"whois\""));
    assert!(whois.contains("href=\"/summary/localhost/\""));
    assert!(whois.contains("value=\"whois\" selected>"));
    let failed = html(&client, base, "/route/localhost/fail").await;
    assert!(failed.contains("<pre>backend unavailable (HTTP 502 Bad Gateway)</pre>"));
    let failed_whois = html(&client, base, "/whois/%0A").await;
    assert!(failed_whois.contains("<pre>"));
    assert!(!failed_whois.contains("<p>"));
    assert_eq!(
        client
            .get(format!("{base}/summary/unknown/"))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );

    for (path, mime) in [
        ("/favicon.ico", "image/x-icon"),
        ("/robots.txt", "text/plain"),
        ("/static/app.css", "text/css"),
        ("/static/goto.js", "text/javascript"),
        ("/static/bgpmap.js", "text/javascript"),
        ("/static/sortTable.js", "text/javascript"),
        (
            "/static/jsdelivr/npm/bootstrap@4.5.1/dist/css/bootstrap.min.css",
            "text/css",
        ),
        (
            "/static/jsdelivr/npm/jquery@3.5.1/dist/jquery.min.js",
            "text/javascript",
        ),
        (
            "/static/jsdelivr/npm/bootstrap@4.5.1/dist/js/bootstrap.min.js",
            "text/javascript",
        ),
        (
            "/static/jsdelivr/npm/viz.js@2.1.2/viz.min.js",
            "text/javascript",
        ),
        (
            "/static/jsdelivr/npm/viz.js@2.1.2/lite.render.js",
            "text/javascript",
        ),
    ] {
        let response = client.get(format!("{base}{path}")).send().await.unwrap();
        assert_eq!(response.status(), 200, "GET {path}");
        assert_eq!(response.headers()["content-type"], mime, "GET {path}");
        assert!(!response.bytes().await.unwrap().is_empty());
    }
    assert_eq!(
        client
            .get(format!("{base}/static/missing.js"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    proxy_task.abort();
    whois_task.abort();
}
