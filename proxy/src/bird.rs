use anyhow::{anyhow, Result};
use std::io::{BufReader, Write};
use crate::settings::Settings;
use tracing::debug;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(not(unix))]
use std::net::TcpStream;

#[cfg(not(unix))]
type UnixStream = TcpStream;

const MAX_LINE_SIZE: usize = 1024;

/// Check if a byte is numeric
fn is_numeric(b: u8) -> bool {
    b >= b'0' && b <= b'9'
}

/// Read a line from bird socket, removing preceding status number
/// Returns if there are more lines
fn bird_read_line(reader: &mut BufReader<UnixStream>, output: &mut Vec<u8>) -> Result<bool> {
    let mut line = Vec::new();
    
    // Read line byte by byte up to MAX_LINE_SIZE
    for _ in 0..MAX_LINE_SIZE {
        let mut byte = [0u8; 1];
        match std::io::Read::read_exact(reader, &mut byte) {
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Err(e) => {
                output.extend_from_slice(e.to_string().as_bytes());
                return Ok(false);
            }
        }
    }

    debug!("Bird raw line: {:?}", String::from_utf8_lossy(&line));

    // Remove preceding status number if present
    if line.len() > 4 
        && is_numeric(line[0]) 
        && is_numeric(line[1]) 
        && is_numeric(line[2]) 
        && is_numeric(line[3]) 
    {
        // There is a status number at beginning, remove first 5 bytes (4 digits + space)
        if line.len() > 6 {
            output.extend_from_slice(&line[5..]);
        }
        // Return true if status is not 0, 8, or 9 (meaning more lines follow)
        Ok(line[0] != b'0' && line[0] != b'8' && line[0] != b'9')
    } else {
        // No status number, output the line (skip first byte which might be a space)
        if line.len() > 1 {
            output.extend_from_slice(&line[1..]);
        }
        Ok(true)
    }
}

/// Write a command to bird socket
fn bird_write_line(stream: &mut UnixStream, command: &str) -> Result<()> {
    stream.write_all(format!("{}\n", command).as_bytes())?;
    stream.flush()?;
    Ok(())
}

/// Connect to BIRD socket - Unix socket on Unix systems, TCP fallback on others
#[cfg(unix)]
fn connect_to_bird(socket_path: &str) -> Result<UnixStream> {
    UnixStream::connect(socket_path)
        .map_err(|e| anyhow!("Failed to connect to BIRD Unix socket: {}", e))
}

#[cfg(not(unix))]
fn connect_to_bird(socket_path: &str) -> Result<UnixStream> {
    // On non-Unix systems, treat the socket_path as host:port for TCP connection
    let addr = if socket_path.contains(':') {
        socket_path.to_string()
    } else {
        format!("127.0.0.1:{}", socket_path)
    };
    
    TcpStream::connect(&addr)
        .map_err(|e| anyhow!("Failed to connect to BIRD TCP socket: {}", e))
}

/// Execute a BIRD command and return the output
pub async fn execute_bird_command(query: &str) -> Result<String> {
    let settings = Settings::global();
    
    // Connect to BIRD socket
    let mut stream = connect_to_bird(&settings.bird_socket)?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| anyhow!("Failed to clone stream: {}", e))?);
    
    // Read initial greeting
    let mut temp_output = Vec::new();
    bird_read_line(&mut reader, &mut temp_output)?;
    
    // Send restrict command
    bird_write_line(&mut stream, "restrict")?;
    
    // Read restriction confirmation
    let mut restrict_output = Vec::new();
    bird_read_line(&mut reader, &mut restrict_output)?;
    
    let restrict_response = String::from_utf8_lossy(&restrict_output);
    if !restrict_response.contains("Access restricted") {
        return Err(anyhow!("Could not verify that bird access was restricted"));
    }
    
    // Send the actual query
    bird_write_line(&mut stream, query)?;
    
    // Read the response
    let mut output = Vec::new();
    while bird_read_line(&mut reader, &mut output)? {
        // Continue reading until no more lines
    }
    
    let result = String::from_utf8_lossy(&output).to_string();
    debug!("Bird command '{}' output: {}", query, result);
    
    Ok(result)
} 