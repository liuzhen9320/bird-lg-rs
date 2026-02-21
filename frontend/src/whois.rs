use anyhow::{anyhow, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use crate::settings::Settings;

// Adds the default whois port (43) if not specified.
// Handles IPv4, IPv6 (bare and bracketed), and domain names.
fn add_default_whois_port(server: &str) -> String {
    // Check if port is already specified
    // For IPv6 addresses, they may be in brackets like [::1] or [::1]:43
    // For IPv4/domain, they look like 192.0.2.1 or example.com
    
    if server.starts_with('[') {
        // IPv6 in brackets: [::1] or [::1]:43
        if let Some(closing_bracket) = server.find(']') {
            if closing_bracket == server.len() - 1 {
                // No port after closing bracket: [::1]
                return format!("{}:43", server);
            }
            // Has something after closing bracket
            if server[closing_bracket + 1..].starts_with(':') {
                // Already has port: [::1]:43
                return server.to_string();
            }
        }
        // Malformed, but add port anyway
        return format!("{}:43", server);
    }
    
    // Not in brackets - could be IPv6 bare, IPv4, or domain
    if let Some(colon_pos) = server.rfind(':') {
        let after_colon = &server[colon_pos + 1..];
        if after_colon.parse::<u16>().is_ok() {
            // Valid port already present
            return server.to_string();
        }
        // Colon exists but not a valid port - must be IPv6 bare format (::1)
        return format!("[{}]:43", server);
    }
    
    // No colon - IPv4, domain, or localhost
    format!("{}:43", server)
}

pub async fn query(target: &str) -> Result<String> {
    let settings = Settings::global();
    
    // Validate and prepare whois server address
    let whois_server = &settings.whois_server;
    let server_addr = add_default_whois_port(whois_server);
    
    // Connect to whois server with timeout
    let stream = timeout(
        Duration::from_secs(10),
        TcpStream::connect(&server_addr)
    ).await
    .map_err(|_| anyhow!("Connection timeout to whois server: {}", server_addr))?
    .map_err(|e| anyhow!("Failed to connect to whois server {}: {}", server_addr, e))?;
    
    let mut stream = stream;
    
    // Send query
    let query_line = format!("{}\r\n", target);
    stream.write_all(query_line.as_bytes()).await
        .map_err(|e| anyhow!("Failed to send query to whois server: {}", e))?;
    
    // Read response with timeout
    let read_result = timeout(
        Duration::from_secs(30),
        read_whois_response(stream)
    ).await
    .map_err(|_| anyhow!("Read timeout from whois server"))?;
    
    read_result
}

async fn read_whois_response(stream: TcpStream) -> Result<String> {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    let mut result = String::new();
    
    while let Some(line) = lines.next_line().await
        .map_err(|e| anyhow!("Failed to read from whois server: {}", e))? {
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