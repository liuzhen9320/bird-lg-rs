use crate::settings::Settings;
use anyhow::{anyhow, Result};
use reqwest::{
    header::{HeaderMap, AUTHORIZATION},
    Client,
};
use std::net::IpAddr;
use std::time::Duration;

fn proxy_url(server: &str, port: u16, endpoint: &str) -> String {
    let host = match server.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("[{}]", server),
        _ => server.to_string(),
    };
    format!("http://{}:{}/{}", host, port, endpoint)
}

/// Validate that all requested servers are in the configured server list
pub fn validate_servers(servers: &[String]) -> Result<()> {
    let settings = Settings::global();

    for server in servers {
        if !settings.servers.contains(server) {
            return Err(anyhow!("request failed: invalid server"));
        }
    }

    Ok(())
}

pub async fn bird_query(server: &str, command: &str) -> Result<String> {
    let settings = Settings::global();
    let client = Client::new();

    let url = proxy_url(server, settings.proxy_port, "bird");

    let mut request = client
        .get(&url)
        .query(&[("q", command)])
        .timeout(Duration::from_secs(settings.timeout));

    // Add authorization header if auth is enabled
    if settings.auth_enabled {
        if let Some(token) = &settings.auth_token {
            let mut headers = HeaderMap::new();
            let header_value = format!("Bearer {}", token)
                .parse()
                .map_err(|e| anyhow!("Invalid auth token: {}", e))?;
            headers.insert(AUTHORIZATION, header_value);
            request = request.headers(headers);
        }
    }

    read_proxy_response(request.send().await?).await
}

pub async fn traceroute_query(server: &str, target: &str) -> Result<String> {
    let settings = Settings::global();
    let client = Client::new();

    let url = proxy_url(server, settings.proxy_port, "traceroute");

    let mut request = client
        .get(&url)
        .query(&[("q", target)])
        .timeout(Duration::from_secs(settings.timeout));

    // Add authorization header if auth is enabled
    if settings.auth_enabled {
        if let Some(token) = &settings.auth_token {
            let mut headers = HeaderMap::new();
            let header_value = format!("Bearer {}", token)
                .parse()
                .map_err(|e| anyhow!("Invalid auth token: {}", e))?;
            headers.insert(AUTHORIZATION, header_value);
            request = request.headers(headers);
        }
    }

    read_proxy_response(request.send().await?).await
}

async fn read_proxy_response(response: reqwest::Response) -> Result<String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| anyhow!("Could not read proxy response (HTTP {}): {}", status, error))?;
    if status.is_success() {
        Ok(body)
    } else if body.trim().is_empty() {
        Err(anyhow!(
            "Proxy returned HTTP {} with an empty error response",
            status
        ))
    } else {
        Err(anyhow!("{} (HTTP {})", body.trim(), status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_urls_bracket_ipv6_hosts() {
        assert_eq!(
            proxy_url("2001:db8::1", 8000, "bird"),
            "http://[2001:db8::1]:8000/bird"
        );
        assert_eq!(
            proxy_url("proxy.example", 8000, "traceroute"),
            "http://proxy.example:8000/traceroute"
        );
    }

    #[tokio::test]
    async fn proxy_errors_keep_the_explanation_and_success_output_is_unchanged() {
        use axum::http::{Response, StatusCode};

        let response = Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body("Traceroute is unavailable on this node.\n")
            .unwrap();
        let error = read_proxy_response(response.into()).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "Traceroute is unavailable on this node. (HTTP 503 Service Unavailable)"
        );

        let response = Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body("")
            .unwrap();
        assert_eq!(
            read_proxy_response(response.into())
                .await
                .unwrap_err()
                .to_string(),
            "Proxy returned HTTP 502 Bad Gateway with an empty error response"
        );

        let response = Response::new("  original output\n\n");
        assert_eq!(
            read_proxy_response(response.into()).await.unwrap(),
            "  original output\n\n"
        );
    }
}
