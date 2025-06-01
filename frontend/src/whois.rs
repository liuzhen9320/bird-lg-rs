use anyhow::{anyhow, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use crate::settings::Settings;

pub async fn query(target: &str) -> Result<String> {
    let settings = Settings::global();
    
    // Validate and prepare whois server address
    let whois_server = &settings.whois_server;
    let server_addr = if whois_server.contains(':') {
        whois_server.to_string()
    } else {
        format!("{}:43", whois_server)
    };
    
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