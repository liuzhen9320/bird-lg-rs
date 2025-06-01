use crate::Args;
use anyhow::Result;
use std::sync::OnceLock;
use tracing::info;

#[derive(Debug, Clone)]
pub struct Settings {
    pub servers: Vec<String>,
    pub servers_display: Vec<String>,
    pub domain: String,
    pub proxy_port: u16,
    pub whois_server: String,
    pub listen: String,
    pub dns_interface: String,
    pub net_specific_mode: String,
    pub title_brand: String,
    pub navbar_brand: String,
    pub navbar_brand_url: String,
    pub navbar_all_server: String,
    pub navbar_all_url: String,
    pub bgpmap_info: String,
    pub telegram_bot_name: String,
    pub protocol_filter: Vec<String>,
    pub name_filter: String,
    pub timeout: u64,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

impl Settings {
    pub async fn init(args: Args) -> Result<()> {
        // Parse servers with display names
        let mut servers = Vec::new();
        let mut servers_display = Vec::new();

        for server_spec in &args.servers {
            if let Some(angle_pos) = server_spec.find('<') {
                // Display name format: "Display<actual>"
                let display = server_spec[..angle_pos].to_string();
                let actual = server_spec[angle_pos + 1..server_spec.len() - 1].to_string();
                servers_display.push(display);
                servers.push(actual);
            } else {
                // Plain server name - store the original as display name
                servers_display.push(server_spec.clone());
                servers.push(server_spec.clone());
            }
        }

        // Build full server names with domain (only modify servers, not servers_display)
        if !args.domain.is_empty() {
            for i in 0..servers.len() {
                if !servers[i].contains('.') && !servers[i].parse::<std::net::IpAddr>().is_ok() {
                    servers[i] = format!("{}.{}", servers[i], args.domain);
                }
            }
        }

        let settings = Settings {
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
            name_filter: args.name_filter,
            timeout: args.timeout,
        };

        info!("Settings initialized: {:?}", settings);

        SETTINGS.set(settings).map_err(|_| anyhow::anyhow!("Settings already initialized"))?;
        Ok(())
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