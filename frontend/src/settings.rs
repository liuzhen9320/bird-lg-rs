use crate::Args;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use tracing::{debug, info};

#[derive(Clone)]
pub struct Settings {
    // Configured identifiers are kept separate from resolved proxy addresses.
    pub server_ids: Vec<String>,
    pub servers: Vec<String>,
    pub servers_display: Vec<String>,
    server_aliases: HashMap<String, usize>,
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
            .field("server_ids", &self.server_ids)
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

fn parse_server_spec(server_spec: &str) -> Result<(String, String, bool)> {
    let (display_name, actual, explicit_display) =
        if let Some((display_name, remainder)) = server_spec.split_once('<') {
            let actual = remainder
                .strip_suffix('>')
                .ok_or_else(|| anyhow::anyhow!("Invalid server specification: {}", server_spec))?;
            if display_name.trim().is_empty()
                || actual.trim().is_empty()
                || actual.contains('<')
                || actual.contains('>')
            {
                anyhow::bail!("Invalid server specification: {}", server_spec);
            }
            (display_name.to_string(), actual.to_string(), true)
        } else {
            if server_spec.contains('>') {
                anyhow::bail!("Invalid server specification: {}", server_spec);
            }
            (server_spec.to_string(), server_spec.to_string(), false)
        };

    if display_name.contains('+') {
        anyhow::bail!("Server display name cannot contain '+': {}", display_name);
    }
    if actual.contains('+') {
        anyhow::bail!("Server identifier cannot contain '+': {}", actual);
    }

    Ok((display_name, actual, explicit_display))
}

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
                anyhow::anyhow!(
                    "Invalid name filter regex '{}': {}",
                    args.name_filter,
                    error
                )
            })?)
        };

        // Parse servers with display names
        let mut server_ids = Vec::new();
        let mut servers = Vec::new();
        let mut servers_display = Vec::new();
        let mut explicit_display_names = Vec::new();

        debug!(
            "Initializing settings with args.servers: {:?}",
            args.servers
        );
        debug!("Domain: '{}'", args.domain);

        for server_spec in &args.servers {
            debug!("Processing server_spec: '{}'", server_spec);
            let (display_name, actual, explicit_display) = parse_server_spec(server_spec)?;
            if servers_display.contains(&display_name) {
                anyhow::bail!("Duplicate server display name: {}", display_name);
            }
            debug!(
                "Parsed server: display_name='{}', actual='{}'",
                display_name, actual
            );
            servers_display.push(display_name);
            server_ids.push(actual.clone());
            servers.push(actual);
            explicit_display_names.push(explicit_display);
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
                    if !explicit_display_names[i]
                        && servers[i].ends_with(&format!(".{}", args.domain))
                    {
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

        for (index, display_name) in servers_display.iter().enumerate() {
            if servers_display[..index].contains(display_name) {
                anyhow::bail!("Duplicate server display name: {}", display_name);
            }
        }

        let mut server_aliases = HashMap::new();
        for index in 0..servers.len() {
            for alias in [&server_ids[index], &servers_display[index], &servers[index]] {
                if let Some(previous) = server_aliases.insert(alias.clone(), index) {
                    if previous != index {
                        anyhow::bail!("Ambiguous server identifier: {}", alias);
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
            server_ids,
            servers,
            servers_display,
            server_aliases,
            domain: args.domain,
            proxy_port: args.proxy_port,
            whois_server: args.whois,
            listen: args.listen,
            dns_interface: args.dns_interface,
            net_specific_mode: args.net_specific_mode,
            navbar_brand: if args.navbar_brand.is_empty() {
                args.title_brand.clone()
            } else {
                args.navbar_brand
            },
            title_brand: args.title_brand,
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

    pub fn all_server_ids(&self) -> String {
        self.server_ids.join("+")
    }

    pub fn get_server_id(&self, server: &str) -> &str {
        &self.server_ids[self.server_aliases[server]]
    }

    pub fn resolve_servers_from_display_names(&self, display_names: &str) -> Result<Vec<String>> {
        if display_names.is_empty() {
            anyhow::bail!("No servers specified");
        }

        let servers = display_names
            .split('+')
            .map(|name| {
                self.server_aliases
                    .get(name)
                    .map(|&index| self.servers[index].clone())
                    .ok_or_else(|| anyhow::anyhow!("Unknown server: {}", name))
            })
            .collect::<Result<Vec<_>>>()?;

        if servers.is_empty() {
            anyhow::bail!("No servers specified");
        }

        Ok(servers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_args;

    #[test]
    fn server_specs_require_complete_unambiguous_aliases() {
        assert_eq!(
            parse_server_spec("Edge<edge.example>").unwrap(),
            ("Edge".to_string(), "edge.example".to_string(), true)
        );
        assert_eq!(
            parse_server_spec("edge.example").unwrap(),
            (
                "edge.example".to_string(),
                "edge.example".to_string(),
                false
            )
        );

        for server_spec in [
            "edge<",
            "edge<actual",
            "<actual>",
            "edge<>",
            "edge<actual>>",
            "edge>actual",
            "edge+other",
        ] {
            assert!(
                parse_server_spec(server_spec).is_err(),
                "accepted {server_spec:?}"
            );
        }
    }

    #[test]
    fn explicit_aliases_survive_domain_processing_and_duplicates_fail() {
        let settings = Settings::from_args(test_args(&[
            "bird-lg-rs",
            "--servers=Display<edge.example>",
            "--domain=example",
        ]))
        .unwrap();
        assert_eq!(settings.servers, ["edge.example"]);
        assert_eq!(settings.servers_display, ["Display"]);

        let error =
            Settings::from_args(test_args(&["bird-lg-rs", "--servers=edge,edge"])).unwrap_err();
        assert_eq!(error.to_string(), "Duplicate server display name: edge");

        let error = Settings::from_args(test_args(&[
            "bird-lg-rs",
            "--servers=edge,edge.example",
            "--domain=example",
        ]))
        .unwrap_err();
        assert_eq!(error.to_string(), "Duplicate server display name: edge");
    }

    #[test]
    fn upstream_ids_and_legacy_names_resolve_to_the_same_configured_proxy() {
        let settings = Settings::from_args(test_args(&[
            "bird-lg-rs",
            "--servers=上海<edge>,Core<core>",
            "--domain=example.net",
        ]))
        .unwrap();
        assert_eq!(settings.server_ids, ["edge", "core"]);
        assert_eq!(settings.servers, ["edge.example.net", "core.example.net"]);
        assert_eq!(settings.all_server_ids(), "edge+core");
        for names in [
            "edge+core",
            "上海+Core",
            "edge.example.net+core.example.net",
        ] {
            assert_eq!(
                settings.resolve_servers_from_display_names(names).unwrap(),
                settings.servers
            );
        }
        assert_eq!(settings.get_server_id("edge.example.net"), "edge");
        assert!(settings
            .resolve_servers_from_display_names("unknown")
            .is_err());
        assert!(settings.resolve_servers_from_display_names("").is_err());
    }

    #[test]
    fn identifiers_cannot_collide_with_another_nodes_old_links() {
        for spec in [
            "first<edge>,edge<core>",
            "first<edge>,second<edge>",
            "Display<edge+core>",
        ] {
            assert!(
                Settings::from_args(test_args(&["bird-lg-rs", &format!("--servers={spec}")]))
                    .is_err(),
                "accepted {spec}"
            );
        }
        assert!(Settings::from_args(test_args(&[
            "bird-lg-rs",
            "--servers=first<edge>,edge.example.net<core>",
            "--domain=example.net",
        ]))
        .is_err());
    }

    #[test]
    fn navbar_brand_inherits_the_configured_title_unless_overridden() {
        for (navbar, expected) in [("", "Example LG"), ("Example Network", "Example Network")] {
            let settings = Settings::from_args(test_args(&[
                "bird-lg-rs",
                "--servers=edge",
                "--title-brand=Example LG",
                &format!("--navbar-brand={navbar}"),
            ]))
            .unwrap();
            assert_eq!(settings.navbar_brand, expected);
            assert_eq!(settings.navbar_all_server, "All Servers");
        }
    }
}
