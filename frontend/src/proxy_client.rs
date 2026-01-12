use anyhow::{anyhow, Result};
use reqwest::{Client, header::{HeaderMap, AUTHORIZATION}};
use std::time::Duration;
use crate::settings::Settings;

pub async fn bird_query(server: &str, command: &str) -> Result<String> {
    let settings = Settings::global();
    let client = Client::new();
    
    let url = format!("http://{}:{}/bird", server, settings.proxy_port);
    
    let mut request = client
        .get(&url)
        .query(&[("q", command)])
        .timeout(Duration::from_secs(settings.timeout));

    // Add authorization header if auth is enabled
    if settings.auth_enabled {
        if let Some(token) = &settings.auth_token {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
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
    
    let url = format!("http://{}:{}/traceroute", server, settings.proxy_port);
    
    let mut request = client
        .get(&url)
        .query(&[("q", target)])
        .timeout(Duration::from_secs(settings.timeout));

    // Add authorization header if auth is enabled
    if settings.auth_enabled {
        if let Some(token) = &settings.auth_token {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
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