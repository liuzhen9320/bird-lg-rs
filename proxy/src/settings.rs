use crate::Args;
use anyhow::Result;
use ipnet::IpNet;
use std::net::IpAddr;
use std::sync::OnceLock;
use tracing::{info, debug};

#[derive(Debug, Clone)]
pub struct Settings {
    pub bird_socket: String,
    pub listen: String,
    pub allowed_nets: Vec<IpNet>,
    pub traceroute_bin: Option<String>,
    pub traceroute_flags: Vec<String>,
    pub traceroute_raw: bool,
    pub traceroute_max_concurrent: usize,
    pub bird_restrict_cmds: bool,
    pub auth_enabled: bool,
    pub auth_token: Option<String>,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

impl Settings {
    pub async fn init(args: Args) -> Result<()> {
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

        let settings = Settings {
            bird_socket: args.bird,
            listen: args.listen,
            allowed_nets,
            traceroute_bin: args.traceroute_bin,
            traceroute_flags,
            traceroute_raw: args.traceroute_raw,
            traceroute_max_concurrent: args.traceroute_max_concurrent,
            bird_restrict_cmds: args.bird_restrict_cmds,
            auth_enabled: args.auth_enabled,
            auth_token: args.auth_token,
        };

        info!("Settings initialized: {:?}", settings);

        SETTINGS.set(settings).map_err(|_| anyhow::anyhow!("Settings already initialized"))?;
        Ok(())
    }

    pub fn global() -> &'static Settings {
        SETTINGS.get().expect("Settings not initialized")
    }

    pub fn has_access(&self, remote_addr: &str) -> bool {
        // If no allowed networks are specified, allow all
        if self.allowed_nets.is_empty() {
            debug!("allowed_nets is empty");
            return true;
        }

        // Extract IP from remote address (remove port if present)
        let ip_str = if let Some(colon_pos) = remote_addr.rfind(':') {
            let ip_part = &remote_addr[..colon_pos];
            // Remove brackets around IPv6 addresses
            ip_part.trim_start_matches('[').trim_end_matches(']')
        } else {
            remote_addr
        };

        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            for net in &self.allowed_nets {
                if net.contains(&ip) {
                    debug!("allowed ip: {}", ip);
                    return true;
                }
            }
        }

        false
    }
} 
