use crate::Args;
use anyhow::Result;
use ipnet::IpNet;
use std::fmt;
use std::net::IpAddr;
use std::sync::OnceLock;
use tracing::{debug, info};

#[derive(Clone)]
pub struct Settings {
    pub bird_socket: String,
    pub bird_timeout: u64,
    pub bird_max_response_bytes: usize,
    pub listen: String,
    pub allowed_nets: Vec<IpNet>,
    pub traceroute_bin: Option<String>,
    pub traceroute_flags: Vec<String>,
    pub traceroute_raw: bool,
    pub traceroute_max_concurrent: usize,
    pub traceroute_timeout: u64,
    pub traceroute_max_output_bytes: usize,
    pub bird_restrict_cmds: bool,
    pub auth_enabled: bool,
    pub auth_token: Option<String>,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("bird_socket", &self.bird_socket)
            .field("bird_timeout", &self.bird_timeout)
            .field("bird_max_response_bytes", &self.bird_max_response_bytes)
            .field("listen", &self.listen)
            .field("allowed_nets", &self.allowed_nets)
            .field("traceroute_bin", &self.traceroute_bin)
            .field("traceroute_flags", &self.traceroute_flags)
            .field("traceroute_raw", &self.traceroute_raw)
            .field("traceroute_max_concurrent", &self.traceroute_max_concurrent)
            .field("traceroute_timeout", &self.traceroute_timeout)
            .field(
                "traceroute_max_output_bytes",
                &self.traceroute_max_output_bytes,
            )
            .field("bird_restrict_cmds", &self.bird_restrict_cmds)
            .field("auth_enabled", &self.auth_enabled)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

impl Settings {
    pub async fn init(args: Args) -> Result<()> {
        let settings = Self::from_args(args)?;

        info!("Settings initialized");

        SETTINGS
            .set(settings)
            .map_err(|_| anyhow::anyhow!("Settings already initialized"))?;
        Ok(())
    }

    pub(crate) fn from_args(args: Args) -> Result<Self> {
        if args.auth_enabled
            && !matches!(args.auth_token.as_deref(), Some(token) if !token.trim().is_empty())
        {
            anyhow::bail!("Authentication token is required when authentication is enabled");
        }

        if args.traceroute_max_concurrent == 0 {
            anyhow::bail!("Traceroute maximum concurrency must be greater than zero");
        }
        if args.bird_timeout == 0 || args.traceroute_timeout == 0 {
            anyhow::bail!("Execution timeouts must be greater than zero");
        }
        if args.bird_max_response_bytes == 0 || args.traceroute_max_output_bytes == 0 {
            anyhow::bail!("Output size limits must be greater than zero");
        }

        let mut allowed_nets = Vec::new();

        // Parse allowed IPs/networks
        if let Some(allowed) = args.allowed {
            for addr_str in allowed {
                if let Ok(ip) = addr_str.parse::<IpAddr>() {
                    // Single IP address - convert to /32 or /128 network
                    allowed_nets.push(IpNet::from(ip));
                } else if let Ok(net) = addr_str.parse::<IpNet>() {
                    // Network range
                    allowed_nets.push(net);
                } else {
                    anyhow::bail!("Invalid IP address or network: {}", addr_str);
                }
            }
        }

        // Parse traceroute flags
        let traceroute_flags = if let Some(flags) = args.traceroute_flags {
            shlex::split(&flags).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Settings {
            bird_socket: args.bird,
            bird_timeout: args.bird_timeout,
            bird_max_response_bytes: args.bird_max_response_bytes,
            listen: args.listen,
            allowed_nets,
            traceroute_bin: args.traceroute_bin,
            traceroute_flags,
            traceroute_raw: args.traceroute_raw,
            traceroute_max_concurrent: args.traceroute_max_concurrent,
            traceroute_timeout: args.traceroute_timeout,
            traceroute_max_output_bytes: args.traceroute_max_output_bytes,
            bird_restrict_cmds: args.bird_restrict_cmds,
            auth_enabled: args.auth_enabled,
            auth_token: args.auth_token,
        })
    }

    pub fn global() -> &'static Settings {
        SETTINGS.get().expect("Settings not initialized")
    }

    pub fn has_access(&self, remote_ip: IpAddr) -> bool {
        // If no allowed networks are specified, allow all
        if self.allowed_nets.is_empty() {
            debug!("allowed_nets is empty");
            return true;
        }

        for net in &self.allowed_nets {
            if net.contains(&remote_ip) {
                debug!("allowed ip: {}", remote_ip);
                return true;
            }
        }

        false
    }
}
