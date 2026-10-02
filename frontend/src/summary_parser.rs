use crate::templates::{SummaryContext, SummaryRowData};
use crate::urls::query_url;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::collections::HashMap;

// Protocol state to CSS class mapping
fn get_state_map() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("up", "success");
    map.insert("down", "secondary");
    map.insert("start", "danger");
    map.insert("passive", "info");
    map
}

pub fn parse_summary(
    data: &str,
    server_name: String,
    protocol_filter: &[String],
    name_filter: Option<&Regex>,
) -> Result<SummaryContext> {
    let lines: Vec<&str> = data.trim().split('\n').collect();

    if lines.len() <= 1 {
        return Err(anyhow!("Invalid summary data: {}", data.trim()));
    }

    // Extract headers from first line
    let headers: Vec<String> = lines[0].split_whitespace().map(|s| s.to_string()).collect();

    // Parse the bird protocol output using regex
    // Format: Name Proto Table State Since Info
    let line_regex = Regex::new(
        r"^([a-zA-Z0-9_-]+)\s+([a-zA-Z0-9_]+)\s+([a-zA-Z0-9_-]+)\s+([a-zA-Z0-9_]+)\s+([0-9\-\. :]+)(.*)$",
    )?;
    let state_map = get_state_map();

    let mut rows = Vec::new();

    // Upstream sorts the complete raw lines before parsing and filtering.
    let mut data_lines = lines[1..].to_vec();
    data_lines.sort_unstable();
    for line in data_lines {
        if line.is_empty() {
            continue;
        }

        if let Some(captures) = line_regex.captures(line) {
            let name = captures
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let proto = captures
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();

            if !protocol_filter.is_empty()
                && !protocol_filter
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(&proto))
            {
                continue;
            }

            if name_filter.is_some_and(|filter| filter.is_match(&name)) {
                continue;
            }

            let table = captures
                .get(3)
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let state = captures
                .get(4)
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let since = captures
                .get(5)
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let info = captures
                .get(6)
                .map(|m| m.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            let mapped_state = if info.contains("Passive") {
                "info".to_string()
            } else {
                state_map.get(state.as_str()).unwrap_or(&"").to_string()
            };

            rows.push(SummaryRowData {
                detail_url: query_url("detail", &server_name, &name),
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

    Ok(SummaryContext {
        server_name,
        headers,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_protocol_rows_keep_names_states_and_filter_semantics() {
        // Includes the dashed-name and Passive cases in upstream template_test.go.
        let data = "Name Proto Table State Since Info\n\
zebra BGP --- unknown 2025-06-27 Other\n\
ibgp_test-01 BGP --- up 07:16:51.656 Established\n\
passive BGP --- start 2025-06-27 21:23:08 Passive\n\
pipe Pipe --- up 2025-06-27 21:23:08 master4 <=> pipe_v4\n\
malformed row";
        let summary = parse_summary(data, "edge".into(), &[], None).unwrap();
        assert_eq!(
            summary
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["ibgp_test-01", "passive", "pipe", "zebra"]
        );
        assert_eq!(summary.rows[0].since, "07:16:51.656");
        assert_eq!(summary.rows[0].detail_url, "/detail/edge/ibgp_test-01");
        assert_eq!(summary.rows[0].mapped_state, "success");
        assert_eq!(summary.rows[1].mapped_state, "info");
        assert_eq!(summary.rows[2].info, "master4 <=> pipe_v4");
        assert_eq!(summary.rows[3].mapped_state, "");
        let filter = Regex::new("^passive$").unwrap();
        let filtered = parse_summary(data, "edge".into(), &["bgp".into()], Some(&filter)).unwrap();
        assert_eq!(
            filtered
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["ibgp_test-01", "zebra"]
        );
    }
}
