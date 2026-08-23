use crate::settings::Settings;
use anyhow::{anyhow, bail, Result};
use regex::Regex;
use std::process::{ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, OnceLock,
};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::Semaphore;
use tokio::time::{timeout_at, Instant};
use tracing::{info, warn};

#[derive(Debug, Clone)]
struct TracerouteConfig {
    bin: String,
    flags: Vec<String>,
}

#[derive(Debug)]
struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
}

static TRACEROUTE_CONFIG: OnceLock<Option<TracerouteConfig>> = OnceLock::new();
static TRACEROUTE_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

/// Convert command and args to string for display
fn args_to_string(cmd: &str, args: &[String], target: &[String]) -> String {
    let mut combined = vec![cmd.to_string()];
    combined.extend_from_slice(args);
    combined.extend_from_slice(target);
    combined.join(" ")
}

async fn read_limited<R>(
    mut reader: R,
    used_bytes: Arc<AtomicUsize>,
    max_output_bytes: usize,
) -> Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(output);
        }

        used_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes_read)
                    .filter(|total| *total <= max_output_bytes)
            })
            .map_err(|_| anyhow!("Traceroute output exceeded {} bytes", max_output_bytes))?;
        output.extend_from_slice(&buffer[..bytes_read]);
    }
}

async fn terminate_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill().await;
    }
    let _ = child.wait().await;
}

async fn run_command_until(
    cmd: &str,
    args: &[String],
    target: &[String],
    deadline: Instant,
    timeout_duration: Duration,
    max_output_bytes: usize,
) -> Result<ProcessOutput> {
    let mut command = Command::new(cmd);
    command
        .args(args)
        .args(target)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to capture traceroute stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to capture traceroute stderr"))?;
    let used_bytes = Arc::new(AtomicUsize::new(0));

    let result = timeout_at(deadline, async {
        tokio::try_join!(
            read_limited(stdout, Arc::clone(&used_bytes), max_output_bytes),
            read_limited(stderr, used_bytes, max_output_bytes),
            async { child.wait().await.map_err(anyhow::Error::from) },
        )
    })
    .await;

    match result {
        Ok(Ok((stdout, _stderr, status))) => Ok(ProcessOutput { status, stdout }),
        Ok(Err(error)) => {
            terminate_child(&mut child).await;
            Err(error)
        }
        Err(_) => {
            terminate_child(&mut child).await;
            bail!(
                "Traceroute timed out after {} seconds",
                timeout_duration.as_secs()
            )
        }
    }
}

async fn run_command(
    cmd: &str,
    args: &[String],
    target: &[String],
    timeout_duration: Duration,
    max_output_bytes: usize,
) -> Result<ProcessOutput> {
    run_command_until(
        cmd,
        args,
        target,
        Instant::now() + timeout_duration,
        timeout_duration,
        max_output_bytes,
    )
    .await
}

/// Try to execute traceroute with given parameters to test if it works
async fn try_execute(cmd: &str, args: &[String], target: &[String]) -> Result<Vec<u8>> {
    let settings = Settings::global();
    let output = run_command(
        cmd,
        args,
        target,
        Duration::from_secs(settings.traceroute_timeout),
        settings.traceroute_max_output_bytes,
    )
    .await?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(anyhow!("Command failed with status: {}", output.status))
    }
}

/// Test if a traceroute configuration works
async fn traceroute_detect(cmd: &str, args: &[String]) -> bool {
    let target = vec!["127.0.0.1".to_string()];

    match try_execute(cmd, args, &target).await {
        Ok(_) => {
            info!(
                "Traceroute autodetect success: {}",
                args_to_string(cmd, args, &target)
            );
            true
        }
        Err(e) => {
            info!(
                "Traceroute autodetect fail, continuing: {} ({})",
                args_to_string(cmd, args, &target),
                e
            );
            false
        }
    }
}

/// Auto-detect the best available traceroute configuration
pub async fn init() {
    let settings = Settings::global();

    // Initialize semaphore for limiting concurrent traceroute requests
    let semaphore = Semaphore::new(settings.traceroute_max_concurrent);
    TRACEROUTE_SEMAPHORE
        .set(semaphore)
        .expect("Semaphore already initialized");

    let mut detected_config = None;

    // If both bin and flags are set, use them directly
    if !settings.traceroute_flags.is_empty() {
        if let Some(bin) = &settings.traceroute_bin {
            let config = TracerouteConfig {
                bin: bin.clone(),
                flags: settings.traceroute_flags.clone(),
            };
            TRACEROUTE_CONFIG
                .set(Some(config))
                .expect("Config already initialized");
            return;
        }
    }

    // Custom binary tests
    if let Some(ref custom_bin) = settings.traceroute_bin {
        // Try different flag combinations for custom binary
        let flag_sets = vec![
            vec!["-q1".to_string(), "-N32".to_string(), "-w1".to_string()],
            vec!["-q1".to_string(), "-w1".to_string()],
            vec![],
        ];

        for flags in flag_sets {
            if traceroute_detect(custom_bin, &flags).await {
                detected_config = Some(TracerouteConfig {
                    bin: custom_bin.clone(),
                    flags,
                });
                break;
            }
        }
    }

    // If no custom binary worked, try standard tools
    if detected_config.is_none() {
        let tool_configs = vec![
            // MTR
            (
                "mtr",
                vec![
                    "-w".to_string(),
                    "-c1".to_string(),
                    "-Z1".to_string(),
                    "-G1".to_string(),
                    "-b".to_string(),
                ],
            ),
            // Traceroute (Debian style)
            (
                "traceroute",
                vec!["-q1".to_string(), "-N32".to_string(), "-w1".to_string()],
            ),
            // Traceroute (FreeBSD style)
            ("traceroute", vec!["-q1".to_string(), "-w1".to_string()]),
            // Traceroute (basic)
            ("traceroute", vec![]),
        ];

        for (bin, flags) in tool_configs {
            if traceroute_detect(bin, &flags).await {
                detected_config = Some(TracerouteConfig {
                    bin: bin.to_string(),
                    flags,
                });
                break;
            }
        }
    }

    if detected_config.is_none() {
        warn!("Traceroute autodetect failed! Traceroute will be disabled");
    }

    TRACEROUTE_CONFIG
        .set(detected_config)
        .expect("Config already initialized");
}

/// Execute traceroute command
pub async fn execute_traceroute(query: &str) -> Result<String> {
    let settings = Settings::global();

    let config = TRACEROUTE_CONFIG
        .get()
        .ok_or_else(|| anyhow!("Traceroute not initialized"))?
        .as_ref()
        .ok_or_else(|| anyhow!("Traceroute not supported on this node"))?;

    let semaphore = TRACEROUTE_SEMAPHORE
        .get()
        .ok_or_else(|| anyhow!("Traceroute semaphore not initialized"))?;

    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err(anyhow!("Invalid target: query is empty"));
    }
    if trimmed_query.contains(' ') {
        return Err(anyhow!(
            "Invalid target: contains spaces (parameter injection not allowed)"
        ));
    }

    let timeout_duration = Duration::from_secs(settings.traceroute_timeout);
    let deadline = Instant::now() + timeout_duration;
    let _permit = timeout_at(deadline, semaphore.acquire())
        .await
        .map_err(|_| {
            anyhow!(
                "Traceroute timed out after {} seconds",
                settings.traceroute_timeout
            )
        })?
        .map_err(|e| anyhow!("Failed to acquire traceroute semaphore: {}", e))?;

    let target = vec![trimmed_query.to_string()];
    let output = run_command_until(
        &config.bin,
        &config.flags,
        &target,
        deadline,
        timeout_duration,
        settings.traceroute_max_output_bytes,
    )
    .await?;

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);

        if settings.traceroute_raw {
            Ok(output_str.to_string())
        } else {
            let re = Regex::new(r"(?m)^\s*(\d*)\s*\*\n").expect("Invalid regex pattern");
            let mut skipped_counter = 0;

            let processed = re.replace_all(&output_str, |_: &regex::Captures| {
                skipped_counter += 1;
                ""
            });

            let mut result = processed.trim().to_string();
            if skipped_counter > 0 {
                result.push_str(&format!("\n\n{} hops not responding.", skipped_counter));
            }

            Ok(result)
        }
    } else {
        Err(anyhow!(
            "Error executing traceroute: command failed with status: {}",
            output.status
        ))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static MARKER_ID: AtomicUsize = AtomicUsize::new(0);

    struct MarkerPath(PathBuf);

    impl MarkerPath {
        fn new(test_name: &str) -> Self {
            let id = MARKER_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "bird-lg-rs-{}-{}-{}",
                test_name,
                std::process::id(),
                id,
            ));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }
    }

    impl Drop for MarkerPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[tokio::test]
    async fn rejects_oversized_traceroute_output() {
        let args = vec!["-c".to_string()];
        let script = vec!["i=0; while [ $i -lt 200 ]; do printf x; i=$((i + 1)); done".to_string()];

        let error = run_command("/bin/sh", &args, &script, Duration::from_secs(1), 64)
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "Traceroute output exceeded 64 bytes");
    }

    #[tokio::test]
    async fn timeout_terminates_the_traceroute_child() {
        let marker = MarkerPath::new("traceroute-timeout");
        let args = vec!["-c".to_string()];
        let script = vec![format!("sleep 0.2; : > {}", marker.0.display(),)];

        let error = run_command("/bin/sh", &args, &script, Duration::from_millis(20), 64)
            .await
            .unwrap_err();

        assert!(error.to_string().starts_with("Traceroute timed out"));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!marker.0.exists());
    }

    #[tokio::test]
    async fn cancellation_terminates_the_traceroute_child() {
        let marker = MarkerPath::new("traceroute-cancel");
        let marker_path = marker.0.clone();
        let task = tokio::spawn(async move {
            let args = vec!["-c".to_string()];
            let script = vec![format!("sleep 0.2; : > {}", marker_path.display(),)];
            run_command("/bin/sh", &args, &script, Duration::from_secs(1), 64).await
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        task.abort();
        let _ = task.await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!marker.0.exists());
    }
}
