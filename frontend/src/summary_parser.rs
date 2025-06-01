use anyhow::{Result, anyhow};
use regex::Regex;
use std::collections::HashMap;
use crate::templates::{SummaryContext, SummaryRowData};

// Protocol state to CSS class mapping
fn get_state_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("up", "success");
    map.insert("down", "secondary");
    map.insert("start", "danger");
    map.insert("passive", "info");
    map
}

pub fn parse_summary(data: &str, server_name: String) -> Result<SummaryContext> {
    let lines: Vec<&str> = data.trim().split('\n').collect();
    
    if lines.len() <= 1 {
        return Err(anyhow!("Invalid summary data: {}", data.trim()));
    }

    // Extract headers from first line
    let headers: Vec<String> = lines[0]
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    // Parse the bird protocol output using regex
    // Format: Name Proto Table State Since Info
    let line_regex = Regex::new(r"(\w+)\s+(\w+)\s+([\w-]+)\s+(\w+)\s+([0-9\-\. :]+)(.*)")?;
    let state_map = get_state_map();
    
    let mut rows = Vec::new();
    
    // Parse each data line (skip header)
    for line in &lines[1..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(captures) = line_regex.captures(line) {
            let name = captures.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let proto = captures.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            let table = captures.get(3).map(|m| m.as_str()).unwrap_or("").to_string();
            let state = captures.get(4).map(|m| m.as_str()).unwrap_or("").to_string();
            let since = captures.get(5).map(|m| m.as_str()).unwrap_or("").trim().to_string();
            let info = captures.get(6).map(|m| m.as_str()).unwrap_or("").trim().to_string();

            let mapped_state = if info.contains("Passive") {
                "info".to_string()
            } else {
                state_map.get(state.as_str()).unwrap_or(&"secondary").to_string()
            };

            rows.push(SummaryRowData {
                name,
                proto,
                table,
                state,
                mapped_state,
                since,
                info,
            });
        }
    }

    // Sort rows by name for consistent output
    rows.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SummaryContext {
        server_name,
        headers,
        rows,
    })
} 