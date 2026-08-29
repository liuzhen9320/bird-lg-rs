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

    let response = request.send().await?;

    if response.status().is_success() {
        Ok(response.text().await?)
    } else {
        Err(anyhow!("HTTP error: {}", response.status()))
    }
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

    let response = request.send().await?;

    if response.status().is_success() {
        Ok(response.text().await?)
    } else {
        Err(anyhow!("HTTP error: {}", response.status()))
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
}
