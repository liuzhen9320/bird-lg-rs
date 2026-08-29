use crate::Args;
use anyhow::Result;
use regex::Regex;
use std::fmt;
use std::sync::OnceLock;
use tracing::{debug, info};

#[derive(Clone)]
pub struct Settings {
    pub servers: Vec<String>,
    pub servers_display: Vec<String>,
    #[allow(dead_code)]
    pub domain: String,
    pub proxy_port: u16,
    pub whois_server: String,
    pub listen: String,
    #[allow(dead_code)]
    pub dns_interface: String,
    #[allow(dead_code)]
    pub net_specific_mode: String,
    pub title_brand: String,
    pub navbar_brand: String,
    pub navbar_brand_url: String,
    pub navbar_all_server: String,
    #[allow(dead_code)]
    pub navbar_all_url: String,
    #[allow(dead_code)]
    pub bgpmap_info: String,
    #[allow(dead_code)]
    pub telegram_bot_name: String,
    pub protocol_filter: Vec<String>,
    pub name_filter: Option<Regex>,
    pub timeout: u64,
    pub auth_enabled: bool,
    pub auth_token: Option<String>,
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("servers", &self.servers)
            .field("servers_display", &self.servers_display)
            .field("domain", &self.domain)
            .field("proxy_port", &self.proxy_port)
            .field("whois_server", &self.whois_server)
            .field("listen", &self.listen)
            .field("dns_interface", &self.dns_interface)
            .field("net_specific_mode", &self.net_specific_mode)
            .field("title_brand", &self.title_brand)
            .field("navbar_brand", &self.navbar_brand)
            .field("navbar_brand_url", &self.navbar_brand_url)
            .field("navbar_all_server", &self.navbar_all_server)
            .field("navbar_all_url", &self.navbar_all_url)
            .field("bgpmap_info", &self.bgpmap_info)
            .field("telegram_bot_name", &self.telegram_bot_name)
            .field("protocol_filter", &self.protocol_filter)
            .field("name_filter", &self.name_filter)
            .field("timeout", &self.timeout)
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
        if args.servers.is_empty() || args.servers.iter().any(|server| server.trim().is_empty()) {
            anyhow::bail!("At least one non-empty server must be configured");
        }

        if args.auth_enabled
            && !matches!(args.auth_token.as_deref(), Some(token) if !token.trim().is_empty())
        {
            anyhow::bail!("Authentication token is required when authentication is enabled");
        }

        let name_filter = if args.name_filter.is_empty() {
            None
        } else {
            Some(Regex::new(&args.name_filter).map_err(|error| {
                anyhow::anyhow!("Invalid name filter regex '{}': {}", args.name_filter, error)
            })?)
        };

        // Parse servers with display names
        let mut servers = Vec::new();
        let mut servers_display = Vec::new();

        debug!(
            "Initializing settings with args.servers: {:?}",
            args.servers
        );
        debug!("Domain: '{}'", args.domain);

        for server_spec in &args.servers {
            debug!("Processing server_spec: '{}'", server_spec);
            if let Some(angle_pos) = server_spec.find('<') {
                // Display name format: "Display<actual>"
                let display_name = server_spec[..angle_pos].to_string();
                let actual = server_spec[angle_pos + 1..server_spec.len() - 1].to_string();
                debug!(
                    "Found <> format: display_name='{}', actual='{}'",
                    display_name, actual
                );
                servers_display.push(display_name);
                servers.push(actual);
            } else {
                // Plain server name - store the original as display name
                debug!("Plain server name: '{}'", server_spec);
                servers_display.push(server_spec.clone());
                servers.push(server_spec.clone());
            }
        }

        debug!("Before domain processing - servers: {:?}", servers);
        debug!(
            "Before domain processing - servers_display: {:?}",
            servers_display
        );

        // Build full server names with domain (only modify servers, not servers_display)
        if !args.domain.is_empty() {
            for i in 0..servers.len() {
                let original = servers[i].clone();
                if !servers[i].contains('.') && !servers[i].parse::<std::net::IpAddr>().is_ok() {
                    servers[i] = format!("{}.{}", servers[i], args.domain);
                    debug!(
                        "Added domain to servers[{}]: '{}' -> '{}'",
                        i, original, servers[i]
                    );
                } else {
                    debug!(
                        "Skipped domain for servers[{}]: '{}' (already has domain or is IP)",
                        i, original
                    );
                    // If the server name already contains the domain, remove it from display name
                    if servers[i].ends_with(&format!(".{}", args.domain)) {
                        let without_domain = servers[i]
                            .strip_suffix(&format!(".{}", args.domain))
                            .unwrap_or(&servers[i]);
                        servers_display[i] = without_domain.to_string();
                        debug!(
                            "Removed domain from servers_display[{}]: '{}' -> '{}'",
                            i, original, servers_display[i]
                        );
                    }
                }
            }
        }

        debug!("After domain processing - servers: {:?}", servers);
        debug!(
            "After domain processing - servers_display: {:?}",
            servers_display
        );

        Ok(Settings {
            servers,
            servers_display,
            domain: args.domain,
            proxy_port: args.proxy_port,
            whois_server: args.whois,
            listen: args.listen,
            dns_interface: args.dns_interface,
            net_specific_mode: args.net_specific_mode,
            title_brand: args.title_brand,
            navbar_brand: args.navbar_brand,
            navbar_brand_url: args.navbar_brand_url,
            navbar_all_server: args.navbar_all_servers,
            navbar_all_url: args.navbar_all_url,
            bgpmap_info: args.bgpmap_info,
            telegram_bot_name: args.telegram_bot_name,
            protocol_filter: args.protocol_filter.unwrap_or_default(),
            name_filter,
            timeout: args.timeout,
            auth_enabled: args.auth_enabled,
            auth_token: args.auth_token,
        })
    }

    pub fn global() -> &'static Settings {
        SETTINGS.get().expect("Settings not initialized")
    }

    pub fn get_server_display_name(&self, server: &str) -> String {
        for (i, s) in self.servers.iter().enumerate() {
            if s == server {
                return self.servers_display[i].clone();
            }
        }
        server.to_string()
    }

    #[allow(dead_code)]
    pub fn all_servers_string(&self) -> String {
        self.servers.join("+")
    }

    pub fn all_servers_display_string(&self) -> String {
        self.servers_display.join("+")
    }

    pub fn get_server_from_display_name(&self, display_name: &str) -> Option<String> {
        for (i, display) in self.servers_display.iter().enumerate() {
            if display == display_name {
                return Some(self.servers[i].clone());
            }
        }
        None
    }

    pub fn resolve_servers_from_display_names(&self, display_names: &str) -> Vec<String> {
        display_names
            .split('+')
            .filter_map(|display_name| {
                // First try to find by display name
                if let Some(server) = self.get_server_from_display_name(display_name) {
                    Some(server)
                } else {
                    // If not found by display name, check if it's already a server name
                    if self.servers.contains(&display_name.to_string()) {
                        Some(display_name.to_string())
                    } else {
                        None
                    }
                }
            })
            .collect()
    }
}
