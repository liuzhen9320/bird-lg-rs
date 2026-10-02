use crate::urls::query_url;
use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

#[derive(Serialize)]
pub struct Part {
    pub text: String,
    pub url: Option<String>,
}

static ASN: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| Regex::new(r"[0-9]+"));
// Recognition rules from bird-lg-go 4f787e5's smartFormatter. Match raw text
// once, then let Tera escape both text and URLs; never run regexes over HTML.
static ADDRESSES: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?P<domain>[a-zA-Z0-9\-]*\.([a-zA-Z]{2,3}){1,2})(?:\s|$)",
        r"|\[(?P<asn>AS[0-9]+)",
        r"|(?P<ipv4>[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)",
        r"|(?P<ipv6>(?i:(?:[a-f0-9]{0,4}:){3,10}[a-f0-9]{0,4}))",
    ))
});

pub fn format_result(input: &str) -> Result<Vec<Part>> {
    let asn = ASN
        .as_ref()
        .map_err(Clone::clone)
        .context("Failed to compile ASN formatter regex")?;
    let addresses = ADDRESSES
        .as_ref()
        .map_err(Clone::clone)
        .context("Failed to compile address formatter regex")?;
    let mut parts = Vec::new();
    for line in input.split('\n') {
        let asn_line = ["BGP.as_path:", "bgp_path:", "Neighbor AS:", "Local AS:"]
            .iter()
            .any(|prefix| line.trim_start().starts_with(prefix));
        let mut cursor = 0;
        let mut add_link = |start: usize, end: usize, target: String| {
            parts.push(Part {
                text: line[cursor..start].to_string(),
                url: None,
            });
            parts.push(Part {
                text: line[start..end].to_string(),
                url: Some(query_url("whois", "", &target)),
            });
            cursor = end;
        };
        if asn_line {
            for matched in asn.find_iter(line) {
                add_link(
                    matched.start(),
                    matched.end(),
                    format!("AS{}", matched.as_str()),
                );
            }
        } else {
            for captures in addresses.captures_iter(line) {
                let matched = ["domain", "asn", "ipv4", "ipv6"]
                    .iter()
                    .find_map(|name| captures.name(name))
                    .context("Address regex matched without a target")?;
                add_link(matched.start(), matched.end(), matched.as_str().to_string());
            }
        }
        parts.push(Part {
            text: format!("{}\n", &line[cursor..]),
            url: None,
        });
    }
    Ok(parts)
}
