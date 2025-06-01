use regex::Regex;
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Clone)]
pub struct RouteAttrs {
    attrs: HashMap<String, String>,
}

impl RouteAttrs {
    pub fn new() -> Self {
        Self {
            attrs: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.attrs.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.attrs.get(key)
    }

    pub fn len(&self) -> usize {
        self.attrs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<String, String> {
        self.attrs.iter()
    }
}

#[derive(Debug, Clone)]
pub struct RoutePoint {
    pub perform_lookup: bool,
    pub attrs: RouteAttrs,
}

impl RoutePoint {
    pub fn new() -> Self {
        Self {
            perform_lookup: false,
            attrs: RouteAttrs::new(),
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct RouteEdgeKey {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Clone)]
pub struct RouteEdgeValue {
    pub label: Vec<String>,
    pub attrs: RouteAttrs,
}

impl RouteEdgeValue {
    pub fn new() -> Self {
        Self {
            label: Vec::new(),
            attrs: RouteAttrs::new(),
        }
    }
}

#[derive(Debug)]
pub struct RouteGraph {
    points: HashMap<String, RoutePoint>,
    edges: HashMap<RouteEdgeKey, RouteEdgeValue>,
}

impl RouteGraph {
    pub fn new() -> Self {
        Self {
            points: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    pub fn add_point(&mut self, name: String, perform_lookup: bool, mut attrs: RouteAttrs) {
        let mut point = self.points.get(&name).cloned().unwrap_or_else(RoutePoint::new);
        point.perform_lookup = perform_lookup;
        
        // Merge attributes
        for (k, v) in attrs.iter() {
            point.attrs.insert(k.clone(), v.clone());
        }
        
        self.points.insert(name, point);
    }

    pub fn add_edge(&mut self, src: String, dest: String, label: String, mut attrs: RouteAttrs) {
        let key = RouteEdgeKey { src, dest };
        let mut edge = self.edges.get(&key).cloned().unwrap_or_else(RouteEdgeValue::new);
        
        if !label.is_empty() {
            edge.label.push(label);
        }
        
        // Merge attributes
        for (k, v) in attrs.iter() {
            edge.attrs.insert(k.clone(), v.clone());
        }
        
        self.edges.insert(key, edge);
    }

    fn escape(&self, s: &str) -> String {
        // Escape special characters for Graphviz DOT syntax
        let escaped = s
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t");
        format!("\"{}\"", escaped)
    }

    fn attrs_to_string(&self, attrs: &RouteAttrs) -> String {
        if attrs.is_empty() {
            return String::new();
        }

        let attr_strings: Vec<String> = attrs
            .iter()
            .map(|(k, v)| {
                let escaped_k = k.replace("\"", "\\\"");
                let escaped_v = v.replace("\"", "\\\"").replace("\n", "\\n");
                format!("{}=\"{}\"", escaped_k, escaped_v)
            })
            .collect();

        format!("[{}]", attr_strings.join(","))
    }

    pub fn to_graphviz(&self) -> String {
        let mut result = String::new();
        let mut asn_cache = ASNCache::new();

        // Add graph attributes
        result.push_str("  rankdir=LR;\n");
        result.push_str("  node [shape=box];\n");

        // Add points
        for (name, point) in &self.points {
            let representation = if point.perform_lookup {
                asn_cache.lookup(name)
            } else {
                name.clone()
            };

            let mut attrs_copy = point.attrs.clone();
            attrs_copy.insert("label".to_string(), representation);

            let attrs_str = self.attrs_to_string(&attrs_copy);
            let attrs_part = if attrs_str.is_empty() { 
                String::new() 
            } else { 
                format!(" {}", attrs_str) 
            };

            result.push_str(&format!(
                "  {}{};\n",
                self.escape(name),
                attrs_part
            ));
        }

        // Add edges
        for (key, edge) in &self.edges {
            let mut attrs_copy = edge.attrs.clone();
            if !edge.label.is_empty() {
                attrs_copy.insert("label".to_string(), edge.label.join("\\n"));
            }
            
            let attrs_str = self.attrs_to_string(&attrs_copy);
            let attrs_part = if attrs_str.is_empty() { 
                String::new() 
            } else { 
                format!(" {}", attrs_str) 
            };
            
            result.push_str(&format!(
                "  {} -> {}{};\n",
                self.escape(&key.src),
                self.escape(&key.dest),
                attrs_part
            ));
        }

        format!("digraph {{\n{}}}\n", result)
    }

    // Helper methods for testing
    #[cfg(test)]
    pub fn get_point(&self, name: &str) -> Option<&RoutePoint> {
        self.points.get(name)
    }

    #[cfg(test)]
    pub fn get_edge(&self, src: &str, dest: &str) -> Option<&RouteEdgeValue> {
        let key = RouteEdgeKey {
            src: src.to_string(),
            dest: dest.to_string(),
        };
        self.edges.get(&key)
    }
}

// ASN Cache for lookup functionality
#[derive(Debug)]
pub struct ASNCache {
    cache: HashMap<String, String>,
}

impl ASNCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn lookup(&mut self, asn: &str) -> String {
        if let Some(cached_value) = self.cache.get(asn) {
            return cached_value.clone();
        }

        // For now, use simplified implementation
        // In a full implementation, this would do DNS/WHOIS lookups
        let result = format!("AS{}", asn);
        self.cache.insert(asn.to_string(), result.clone());
        result
    }
}

fn make_edge_attrs(preferred: bool) -> RouteAttrs {
    let mut attrs = RouteAttrs::new();
    attrs.insert("fontsize".to_string(), "12.0".to_string());
    if preferred {
        attrs.insert("color".to_string(), "red".to_string());
    }
    attrs
}

fn make_point_attrs(preferred: bool) -> RouteAttrs {
    let mut attrs = RouteAttrs::new();
    if preferred {
        attrs.insert("color".to_string(), "red".to_string());
    }
    attrs
}

fn bird_route_to_graph(servers: &[String], responses: &[String], target: &str) -> RouteGraph {
    let mut graph = RouteGraph::new();
    
    // Add target point
    let mut target_attrs = RouteAttrs::new();
    target_attrs.insert("color".to_string(), "red".to_string());
    target_attrs.insert("shape".to_string(), "diamond".to_string());
    graph.add_point(target.to_string(), false, target_attrs);

    // Compile regex patterns
    let protocol_name_re = Regex::new(r"\[(.*?) .*\]").unwrap();
    let route_split_re = Regex::new(r"(unicast|blackhole|unreachable|prohibited)").unwrap();
    let route_via_re = Regex::new(r"(?m)^\t(via .*?)$").unwrap();
    let route_as_path_re = Regex::new(r"(?m)^\tBGP\.as_path: (.*?)$").unwrap();

    for (server_id, server) in servers.iter().enumerate() {
        if let Some(response) = responses.get(server_id) {
            if response.is_empty() {
                continue;
            }

            // Add server point
            let mut server_attrs = RouteAttrs::new();
            server_attrs.insert("color".to_string(), "blue".to_string());
            server_attrs.insert("shape".to_string(), "box".to_string());
            graph.add_point(server.clone(), false, server_attrs);

            // Split routes
            let routes: Vec<&str> = route_split_re.split(response).collect();

            for (route_index, route) in routes.iter().enumerate() {
                if route_index == 0 {
                    continue;
                }

                let route_preferred = route.contains('*');
                let mut via = String::new();
                let mut paths = Vec::new();
                let mut protocol_name = String::new();

                // Extract via information
                if let Some(captures) = route_via_re.captures(route) {
                    if let Some(via_match) = captures.get(1) {
                        via = via_match.as_str().trim().to_string();
                    }
                }

                // Extract AS path
                if let Some(captures) = route_as_path_re.captures(route) {
                    if let Some(path_match) = captures.get(1) {
                        let path_string = path_match.as_str().trim();
                        if !path_string.is_empty() {
                            paths = path_string
                                .split_whitespace()
                                .map(|p| p.trim_start_matches('(').trim_end_matches(')').to_string())
                                .collect();
                        }
                    }
                }

                // Extract protocol name
                if let Some(captures) = protocol_name_re.captures(route) {
                    if let Some(protocol_match) = captures.get(1) {
                        protocol_name = protocol_match.as_str().trim().to_string();
                        if route_preferred {
                            protocol_name = format!("{}*", protocol_name);
                        }
                    }
                }

                if paths.is_empty() {
                    // Direct connection
                    let label = format!("{}\n{}", protocol_name, via).trim().to_string();
                    graph.add_edge(
                        server.clone(),
                        target.to_string(),
                        label,
                        make_edge_attrs(route_preferred),
                    );
                    continue;
                }

                // Process AS path
                for (i, path_asn) in paths.iter().enumerate() {
                    let (src, label) = if i == 0 {
                        (server.clone(), format!("{}\n{}", protocol_name, via).trim().to_string())
                    } else {
                        (paths[i - 1].clone(), String::new())
                    };
                    let dst = path_asn.clone();

                    graph.add_edge(src, dst.clone(), label, make_edge_attrs(route_preferred));
                    graph.add_point(dst, true, make_point_attrs(route_preferred));
                }

                // Last AS to destination
                if let Some(last_as) = paths.last() {
                    graph.add_edge(
                        last_as.clone(),
                        target.to_string(),
                        String::new(),
                        make_edge_attrs(route_preferred),
                    );
                }
            }
        }
    }

    graph
}

pub fn bird_route_to_graphviz(servers: &[String], responses: &[String], target: &str) -> String {
    let graph = bird_route_to_graph(servers, responses, target);
    graph.to_graphviz()
}

#[allow(dead_code)]
pub fn debug_graphviz_generation() {
    let servers = vec!["test_server".to_string()];
    let responses = vec![r#"Table master4:
172.20.0.53/32       unicast [ibgp_sjc2 2023-04-29 from fd86:bad:11b7:22::1] * (100/38) [AS4242423914i]
	via 169.254.108.122 on igp-sjc2
	Type: BGP univ
	BGP.origin: IGP
	BGP.as_path: 4242423914
	BGP.next_hop: 172.20.229.122
                     unicast [miaotony_2688 2023-04-29 from fe80::2688] (100) [AS4242423914i]
                     
	via 172.23.6.6 on dn42las-miaoton
	Type: BGP univ
	BGP.origin: IGP
	BGP.as_path: 4242422688 4242423914
	BGP.next_hop: 172.23.6.6"#.to_string()];
    
    println!("=== BGP MAP DEBUG ===");
    println!("Input servers: {:?}", servers);
    println!("Input responses length: {}", responses[0].len());
    
    // Generate the DOT graph (matches handlers.rs logic)
    let dot_graph = bird_route_to_graphviz(&servers, &responses, "172.20.0.53");
    
    println!("\n=== Generated DOT Graph ===");
    println!("{}", dot_graph);
    println!("DOT length: {} characters", dot_graph.len());
    
    // Generate base64 (matches handlers.rs logic)
    let base64_result = general_purpose::STANDARD.encode(&dot_graph);
    
    println!("\n=== Base64 Output ===");
    println!("Base64 length: {} characters", base64_result.len());
    println!("Base64 result: {}", base64_result);
    
    // Verify round-trip
    println!("\n=== Round-trip Verification ===");
    if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(&base64_result) {
        if let Ok(decoded_string) = String::from_utf8(decoded_bytes) {
            println!("✅ Successfully decoded base64");
            println!("Decoded length: {} characters", decoded_string.len());
            println!("Matches original: {}", decoded_string == dot_graph);
            
            if decoded_string != dot_graph {
                println!("❌ MISMATCH DETECTED!");
                println!("Original:\n{}", dot_graph);
                println!("Decoded:\n{}", decoded_string);
            }
        } else {
            println!("❌ Failed to convert decoded bytes to UTF-8 string");
        }
    } else {
        println!("❌ Failed to decode base64");
    }
    
    // Check for common issues
    println!("\n=== Validation Checks ===");
    let checks = vec![
        ("Starts with 'digraph {'", dot_graph.starts_with("digraph {")),
        ("Ends with '}\n'", dot_graph.ends_with("}\n")),
        ("Contains rankdir", dot_graph.contains("rankdir=LR")),
        ("Contains node directive", dot_graph.contains("node [shape=box]")),
        ("All lines end properly", dot_graph.lines().all(|line| !line.contains("->") || line.trim().ends_with(';'))),
    ];
    
    for (check_name, passed) in checks {
        println!("{} {}", if passed { "✅" } else { "❌" }, check_name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bird_route_to_graph_xss() {
        let fake_result = r#"<script>alert("evil!")</script>"#;
        let result = bird_route_to_graphviz(
            &[String::from("alpha")],
            &[fake_result.to_string()],
            fake_result,
        );

        // Decode the base64 result to check for XSS
        let decoded = String::from_utf8(general_purpose::STANDARD.decode(result).unwrap()).unwrap();
        assert!(!decoded.contains(fake_result), "XSS injection succeeded: {}", decoded);
    }

    #[test]
    fn test_bird_route_to_graph() {
        let input = r#"Table master4:
172.20.0.53/32       unicast [ibgp_sjc2 2023-04-29 from fd86:bad:11b7:22::1] * (100/38) [AS4242423914i]
	via 169.254.108.122 on igp-sjc2
	Type: BGP univ
	BGP.origin: IGP
	BGP.as_path: 4242423914
	BGP.next_hop: 172.20.229.122
	BGP.med: 50
	BGP.local_pref: 100
	BGP.community: (64511,1) (64511,24) (64511,34)
	BGP.large_community: (4242421080, 101, 44) (4242421080, 103, 122) (4242421080, 104, 1)
                     unicast [miaotony_2688 2023-04-29 from fe80::2688] (100) [AS4242423914i]
                     
	via 172.23.6.6 on dn42las-miaoton
	Type: BGP univ
	BGP.origin: IGP
	BGP.as_path: 4242422688 4242423914
	BGP.next_hop: 172.23.6.6
	BGP.med: 50
	BGP.local_pref: 100
	BGP.community: (64511,3) (64511,24) (64511,34)
	BGP.large_community: (4242421080, 104, 1) (4242421080, 101, 44) (4242421080, 103, 126)"#;

        let result = bird_route_to_graph(&[String::from("node")], &[input.to_string()], "target");

        // Source node must exist
        assert!(result.get_point("node").is_some(), "Result doesn't contain point node");
        
        // Last hop must exist
        assert!(result.get_point("4242423914").is_some(), "Result doesn't contain point 4242423914");
        
        // Destination must exist
        assert!(result.get_point("target").is_some(), "Result doesn't contain point target");

        // Verify that a few paths exist
        assert!(result.get_edge("node", "4242423914").is_some(), "Result doesn't contain edge from node to 4242423914");
        assert!(result.get_edge("node", "4242422688").is_some(), "Result doesn't contain edge from node to 4242422688");
        assert!(result.get_edge("4242422688", "4242423914").is_some(), "Result doesn't contain edge from 4242422688 to 4242423914");
        assert!(result.get_edge("4242423914", "target").is_some(), "Result doesn't contain edge from 4242423914 to target");
    }

    #[test]
    fn test_bird_route_to_graphviz() {
        let input = r#"Table master4:
172.20.0.53/32       unicast [ibgp_sjc2 2023-04-29 from fd86:bad:11b7:22::1] * (100/38) [AS4242423914i]
	via 169.254.108.122 on igp-sjc2
	Type: BGP univ
	BGP.origin: IGP
	BGP.as_path: 4242423914
	BGP.next_hop: 172.20.229.122"#;

        let dot_result = bird_route_to_graphviz(&[String::from("node")], &[input.to_string()], "target");
        let base64_result = general_purpose::STANDARD.encode(&dot_result);
        
        // Decode the base64 result
        let decoded = String::from_utf8(general_purpose::STANDARD.decode(base64_result).unwrap()).unwrap();
        assert!(decoded.contains("digraph {"), "Response is not Graphviz data");
        assert_eq!(decoded, dot_result, "Round-trip encoding/decoding should match");
    }

    #[test]
    fn test_graphviz_syntax_completeness() {
        let input = r#"Table master4:
172.20.0.53/32       unicast [ibgp_sjc2 2023-04-29 from fd86:bad:11b7:22::1] * (100/38) [AS4242423914i]
	via 169.254.108.122 on igp-sjc2
	Type: BGP univ
	BGP.origin: IGP
	BGP.as_path: 4242423914
	BGP.next_hop: 172.20.229.122"#;

        let dot_result = bird_route_to_graphviz(&[String::from("node")], &[input.to_string()], "target");
        let base64_result = general_purpose::STANDARD.encode(&dot_result);
        
        // Decode the base64 result
        let decoded = String::from_utf8(general_purpose::STANDARD.decode(base64_result).unwrap()).unwrap();
        println!("Generated DOT:\n{}", decoded);
        
        // Check that it starts and ends properly
        assert!(decoded.starts_with("digraph {"), "DOT should start with 'digraph {{'");
        assert!(decoded.ends_with("}\n"), "DOT should end with '}}'");
        
        // Check that it contains required elements
        assert!(decoded.contains("rankdir=LR"), "Should contain rankdir directive");
        assert!(decoded.contains("node [shape=box]"), "Should contain node shape directive");
        assert!(decoded.contains("target"), "Should contain target node");
        assert!(decoded.contains("node"), "Should contain server node");
        
        // Check that edges are properly formatted (should contain -> and end with ;)
        let lines: Vec<&str> = decoded.lines().collect();
        for line in &lines {
            if line.contains("->") {
                assert!(line.trim().ends_with(';'), "Edge line should end with semicolon: {}", line);
            }
            if line.contains('[') && !line.contains("node [") {
                assert!(line.trim().ends_with(';'), "Attribute line should end with semicolon: {}", line);
            }
        }
    }

    #[test]
    fn test_debug_output() {
        println!("\n🔍 Running BGP MAP debug output...\n");
        debug_graphviz_generation();
    }
} 