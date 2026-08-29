use crate::settings::Settings;
use anyhow::{anyhow, bail, Result};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::time::timeout;
use tracing::debug;

#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(not(unix))]
use tokio::net::TcpStream;

#[cfg(not(unix))]
type UnixStream = TcpStream;

const MAX_LINE_SIZE: usize = 1024;

/// Check if a byte is numeric
fn is_numeric(b: u8) -> bool {
    b.is_ascii_digit()
}

fn append_bounded(output: &mut Vec<u8>, bytes: &[u8], max_response_bytes: usize) -> Result<()> {
    if bytes.len() > max_response_bytes.saturating_sub(output.len()) {
        bail!("BIRD response exceeded {} bytes", max_response_bytes);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

/// Read a line from the BIRD socket, removing the preceding status number.
/// Returns whether there are more lines.
async fn bird_read_line<R>(
    reader: &mut R,
    output: &mut Vec<u8>,
    max_response_bytes: usize,
) -> Result<bool>
where
    R: AsyncRead + Unpin,
{
    let mut line = Vec::new();

    loop {
        let mut byte = [0u8; 1];
        let bytes_read = reader.read(&mut byte).await?;
        if bytes_read == 0 {
            bail!("Unexpected EOF from BIRD socket");
        }
        if line.len() == MAX_LINE_SIZE {
            bail!("BIRD response line exceeded {} bytes", MAX_LINE_SIZE);
        }

        line.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }

    debug!("Bird raw line: {:?}", String::from_utf8_lossy(&line));

    if line.len() > 4
        && is_numeric(line[0])
        && is_numeric(line[1])
        && is_numeric(line[2])
        && is_numeric(line[3])
    {
        if line.len() > 5 {
            append_bounded(output, &line[5..], max_response_bytes)?;
        }
        Ok(line[0] != b'0' && line[0] != b'8' && line[0] != b'9')
    } else {
        if line.len() > 1 {
            append_bounded(output, &line[1..], max_response_bytes)?;
        }
        Ok(true)
    }
}

async fn bird_write_line<W>(stream: &mut W, command: &str) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    stream.write_all(command.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}

/// Connect to BIRD socket - Unix socket on Unix systems, TCP fallback on others
#[cfg(unix)]
async fn connect_to_bird(socket_path: &str) -> Result<UnixStream> {
    UnixStream::connect(socket_path)
        .await
        .map_err(|e| anyhow!("Failed to connect to BIRD Unix socket: {}", e))
}

#[cfg(not(unix))]
async fn connect_to_bird(socket_path: &str) -> Result<UnixStream> {
    let addr = if socket_path.contains(':') {
        socket_path.to_string()
    } else {
        format!("127.0.0.1:{}", socket_path)
    };

    TcpStream::connect(&addr)
        .await
        .map_err(|e| anyhow!("Failed to connect to BIRD TCP socket: {}", e))
}

async fn execute_bird_exchange(
    socket_path: &str,
    query: &str,
    restrict_cmds: bool,
    max_response_bytes: usize,
) -> Result<String> {
    let stream = connect_to_bird(socket_path).await?;
    let mut reader = BufReader::new(stream);

    let mut greeting = Vec::new();
    bird_read_line(&mut reader, &mut greeting, max_response_bytes).await?;

    if restrict_cmds {
        bird_write_line(reader.get_mut(), "restrict").await?;

        let mut restrict_output = Vec::new();
        bird_read_line(&mut reader, &mut restrict_output, max_response_bytes).await?;

        let restrict_response = String::from_utf8_lossy(&restrict_output);
        if !restrict_response.contains("Access restricted") {
            bail!("Could not verify that bird access was restricted");
        }
    }

    bird_write_line(reader.get_mut(), query).await?;

    let mut output = Vec::new();
    while bird_read_line(&mut reader, &mut output, max_response_bytes).await? {}

    let result = String::from_utf8_lossy(&output).to_string();
    debug!("Bird command '{}' output: {}", query, result);
    Ok(result)
}

async fn execute_bird_command_with_limits(
    socket_path: &str,
    query: &str,
    restrict_cmds: bool,
    execution_timeout: Duration,
    max_response_bytes: usize,
) -> Result<String> {
    timeout(
        execution_timeout,
        execute_bird_exchange(socket_path, query, restrict_cmds, max_response_bytes),
    )
    .await
    .map_err(|_| {
        anyhow!(
            "BIRD command timed out after {} seconds",
            execution_timeout.as_secs()
        )
    })?
}

/// Execute a BIRD command and return the output
pub async fn execute_bird_command(query: &str) -> Result<String> {
    let settings = Settings::global();
    execute_bird_command_with_limits(
        &settings.bird_socket,
        query,
        settings.bird_restrict_cmds,
        Duration::from_secs(settings.bird_timeout),
        settings.bird_max_response_bytes,
    )
    .await
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    use tokio::net::UnixListener;

    static SOCKET_ID: AtomicUsize = AtomicUsize::new(0);

    struct SocketPath(PathBuf);

    impl SocketPath {
        fn new(test_name: &str) -> Self {
            let id = SOCKET_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "bird-lg-rs-{}-{}-{}.sock",
                test_name,
                std::process::id(),
                id,
            ));
            Self(path)
        }
    }

    impl Drop for SocketPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    async fn serve_bird_response(listener: UnixListener, restrict_cmds: bool, response: Vec<u8>) {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(b"0001 BIRD ready\n")
            .await
            .unwrap();

        let mut command = String::new();
        if restrict_cmds {
            reader.read_line(&mut command).await.unwrap();
            assert_eq!(command, "restrict\n");
            reader
                .get_mut()
                .write_all(b"0001 Access restricted\n")
                .await
                .unwrap();
            command.clear();
        }

        reader.read_line(&mut command).await.unwrap();
        assert_eq!(command, "show route\n");
        reader.get_mut().write_all(&response).await.unwrap();
    }

    #[tokio::test]
    async fn reads_a_bounded_bird_response() {
        let socket = SocketPath::new("success");
        let listener = UnixListener::bind(&socket.0).unwrap();
        let server = tokio::spawn(serve_bird_response(
            listener,
            true,
            b"0000 route result\n".to_vec(),
        ));

        let output = execute_bird_command_with_limits(
            socket.0.to_str().unwrap(),
            "show route",
            true,
            Duration::from_secs(1),
            64,
        )
        .await
        .unwrap();

        assert_eq!(output, "route result\n");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_an_oversized_bird_response() {
        let socket = SocketPath::new("oversized");
        let listener = UnixListener::bind(&socket.0).unwrap();
        let response = format!("1000 {}\n", "x".repeat(128)).into_bytes();
        let server = tokio::spawn(serve_bird_response(listener, true, response));

        let error = execute_bird_command_with_limits(
            socket.0.to_str().unwrap(),
            "show route",
            true,
            Duration::from_secs(1),
            64,
        )
        .await
        .unwrap_err();

        assert_eq!(error.to_string(), "BIRD response exceeded 64 bytes");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn times_out_a_stalled_bird_connection() {
        let socket = SocketPath::new("timeout");
        let listener = UnixListener::bind(&socket.0).unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        let error = execute_bird_command_with_limits(
            socket.0.to_str().unwrap(),
            "show route",
            true,
            Duration::from_millis(20),
            64,
        )
        .await
        .unwrap_err();

        assert_eq!(error.to_string(), "BIRD command timed out after 0 seconds");
        server.abort();
    }

    #[tokio::test]
    async fn skips_restrict_exchange_when_disabled() {
        let socket = SocketPath::new("unrestricted");
        let listener = UnixListener::bind(&socket.0).unwrap();
        let server = tokio::spawn(serve_bird_response(
            listener,
            false,
            b"0000 route result\n".to_vec(),
        ));

        let output = execute_bird_command_with_limits(
            socket.0.to_str().unwrap(),
            "show route",
            false,
            Duration::from_secs(1),
            64,
        )
        .await
        .unwrap();

        assert_eq!(output, "route result\n");
        server.await.unwrap();
    }
}
