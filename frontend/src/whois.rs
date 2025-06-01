use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;
use crate::settings::Settings;

pub async fn query(target: &str) -> Result<String> {
    let settings = Settings::global();
    
    // Connect to whois server
    let stream = TcpStream::connect_timeout(
        &format!("{}:43", settings.whois_server).parse()?,
        Duration::from_secs(10),
    )?;
    
    let mut stream = stream;
    
    // Send query
    writeln!(stream, "{}", target)?;
    
    // Read response
    let reader = BufReader::new(&stream);
    let mut result = String::new();
    
    for line in reader.lines() {
        let line = line?;
        result.push_str(&line);
        result.push('\n');
    }
    
    Ok(result)
} 