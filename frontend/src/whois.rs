use crate::settings::Settings;
use anyhow::{anyhow, Result};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

// Adds the default whois port (43) if not specified.
// Handles IPv4, IPv6 (bare and bracketed), and domain names.
fn add_default_whois_port(server: &str) -> Result<String> {
    let server = server.trim();
    if server.is_empty() {
        return Err(anyhow!("Whois server is empty"));
    }

    if let Ok(address) = server.parse::<SocketAddr>() {
        return Ok(address.to_string());
    }
    if let Ok(ip) = server.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, 43).to_string());
    }
    if let Some(ip) = server
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        let ip = ip
            .parse::<IpAddr>()
            .map_err(|_| anyhow!("Invalid bracketed whois server: {}", server))?;
        return Ok(SocketAddr::new(ip, 43).to_string());
    }
    if let Some((host, port)) = server.rsplit_once(':') {
        if host.is_empty() || host.contains(':') {
            return Err(anyhow!("Invalid whois server address: {}", server));
        }
        let port = port
            .parse::<u16>()
            .map_err(|_| anyhow!("Invalid whois server port: {}", server))?;
        return Ok(format!("{}:{}", host, port));
    }

    Ok(format!("{}:43", server))
}

pub async fn query(target: &str) -> Result<String> {
    let settings = Settings::global();

    // Validate and prepare whois server address
    let whois_server = &settings.whois_server;
    let server_addr = add_default_whois_port(whois_server)?;

    // Connect to whois server with timeout
    let stream = timeout(Duration::from_secs(10), TcpStream::connect(&server_addr))
        .await
        .map_err(|_| anyhow!("Connection timeout to whois server: {}", server_addr))?
        .map_err(|e| anyhow!("Failed to connect to whois server {}: {}", server_addr, e))?;

    let mut stream = stream;

    // Send query
    let query_line = format!("{}\r\n", target);
    stream
        .write_all(query_line.as_bytes())
        .await
        .map_err(|e| anyhow!("Failed to send query to whois server: {}", e))?;

    // Read response with timeout
    let read_result = timeout(Duration::from_secs(30), read_whois_response(stream))
        .await
        .map_err(|_| anyhow!("Read timeout from whois server"))?;

    read_result
}

async fn read_whois_response(stream: TcpStream) -> Result<String> {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    let mut result = String::new();

    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| anyhow!("Failed to read from whois server: {}", e))?
    {
        result.push_str(&line);
        result.push('\n');

        // Prevent extremely large responses
        if result.len() > 100_000 {
            result.push_str("\n[Response truncated - too large]\n");
            break;
        }
    }

    if result.is_empty() {
        return Err(anyhow!("Empty response from whois server"));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whois_addresses_handle_ipv6_and_explicit_ports() {
        assert_eq!(
            add_default_whois_port("2001:db8::1").unwrap(),
            "[2001:db8::1]:43"
        );
        assert_eq!(
            add_default_whois_port("[2001:db8::1]:4343").unwrap(),
            "[2001:db8::1]:4343"
        );
        assert_eq!(
            add_default_whois_port("whois.example:4343").unwrap(),
            "whois.example:4343"
        );
        assert_eq!(
            add_default_whois_port("whois.example").unwrap(),
            "whois.example:43"
        );
    }
}
