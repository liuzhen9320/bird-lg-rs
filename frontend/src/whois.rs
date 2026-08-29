use crate::settings::Settings;
use anyhow::{anyhow, Result};
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

const MAX_RESPONSE_BYTES: usize = 100_000;

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

fn validate_whois_target(target: &str) -> Result<&str> {
    if target.chars().any(char::is_control) {
        return Err(anyhow!("Whois target contains control characters"));
    }
    let target = target.trim();
    if target.is_empty() {
        return Err(anyhow!("Whois target is empty"));
    }

    Ok(target)
}

pub async fn query(target: &str) -> Result<String> {
    let settings = Settings::global();
    let target = validate_whois_target(target)?;

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

async fn read_whois_response<R>(stream: R) -> Result<String>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(MAX_RESPONSE_BYTES + 1);
    stream
        .take((MAX_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| anyhow!("Failed to read from whois server: {}", e))?;

    if bytes.is_empty() {
        return Err(anyhow!("Empty response from whois server"));
    }

    let truncated = bytes.len() > MAX_RESPONSE_BYTES;
    bytes.truncate(MAX_RESPONSE_BYTES);
    let response = String::from_utf8_lossy(&bytes);
    let mut result = String::with_capacity(response.len());
    for line in response.lines() {
        result.push_str(line);
        result.push('\n');
    }
    if truncated {
        result.push_str("\n[Response truncated - too large]\n");
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

    #[test]
    fn whois_targets_reject_protocol_line_injection() {
        for target in [
            "",
            "   ",
            "example.net\r\nhelp",
            "example.net\r\n",
            "example.net\0",
        ] {
            assert!(validate_whois_target(target).is_err(), "accepted {target:?}");
        }
        assert_eq!(validate_whois_target(" example.net ").unwrap(), "example.net");
    }

    #[tokio::test]
    async fn whois_response_limit_applies_to_a_single_long_line() {
        let response = vec![b'x'; MAX_RESPONSE_BYTES + 1_000];
        let result = read_whois_response(response.as_slice()).await.unwrap();

        assert!(result.starts_with(&"x".repeat(MAX_RESPONSE_BYTES)));
        assert!(result.ends_with("[Response truncated - too large]\n"));
    }
}
