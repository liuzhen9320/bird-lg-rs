use anyhow::{anyhow, Result};
use regex::Regex;
use std::sync::OnceLock;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tracing::{info, warn};
use crate::settings::Settings;

#[derive(Debug, Clone)]
struct TracerouteConfig {
    bin: String,
    flags: Vec<String>,
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

/// Try to execute traceroute with given parameters to test if it works
async fn try_execute(cmd: &str, args: &[String], target: &[String]) -> Result<Vec<u8>> {
    let mut command = Command::new(cmd);
    command.args(args);
    command.args(target);
    
    let output = command.output().await?;
    
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
            info!("Traceroute autodetect success: {}", args_to_string(cmd, args, &target));
            true
        }
        Err(e) => {
            info!("Traceroute autodetect fail, continuing: {} ({})", 
                  args_to_string(cmd, args, &target), e);
            false
        }
    }
}

/// Auto-detect the best available traceroute configuration
pub async fn init() {
    let settings = Settings::global();
    
    // Initialize semaphore for limiting concurrent traceroute requests
    let semaphore = Semaphore::new(settings.traceroute_max_concurrent);
    TRACEROUTE_SEMAPHORE.set(semaphore).unwrap();

    // If both bin and flags are set, use them directly
    if settings.traceroute_bin.is_some() && !settings.traceroute_flags.is_empty() {
        let config = TracerouteConfig {
            bin: settings.traceroute_bin.as_ref().unwrap().clone(),
            flags: settings.traceroute_flags.clone(),
        };
        TRACEROUTE_CONFIG.set(Some(config)).unwrap();
        return;
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
            ("mtr", vec!["-w".to_string(), "-c1".to_string(), "-Z1".to_string(), "-G1".to_string(), "-b".to_string()]),
            // Traceroute (Debian style)
            ("traceroute", vec!["-q1".to_string(), "-N32".to_string(), "-w1".to_string()]),
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
    
    TRACEROUTE_CONFIG.set(detected_config).unwrap();
    
    // Initialize semaphore for limiting concurrent traceroute requests
    let semaphore = Semaphore::new(settings.traceroute_max_concurrent);
    TRACEROUTE_SEMAPHORE.set(semaphore).unwrap();
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
    
    // Acquire semaphore permit to limit concurrent requests
    let _permit = semaphore.acquire().await
        .map_err(|e| anyhow!("Failed to acquire traceroute semaphore: {}", e))?;
    
    // Execute traceroute
    let args = shlex::split(query.trim())
        .ok_or_else(|| anyhow!("Failed to parse args: invalid shell syntax"))?;
    
    // Execute traceroute
    let result = try_execute(&config.bin, &config.flags, &args).await
        .map_err(|e| anyhow!("Error executing traceroute: {}", e))?;
    
    let output = String::from_utf8_lossy(&result);
    
    if settings.traceroute_raw {
        Ok(output.to_string())
    } else {
        // Process output to remove unresponsive hops and count them
        let re = Regex::new(r"(?m)^\s*(\d*)\s*\*\n").unwrap();
        let mut skipped_counter = 0;
        
        let processed = re.replace_all(&output, |_: &regex::Captures| {
            skipped_counter += 1;
            ""
        });
        
        let mut result = processed.trim().to_string();
        if skipped_counter > 0 {
            result.push_str(&format!("\n\n{} hops not responding.", skipped_counter));
        }
        
        Ok(result)
    }
} 