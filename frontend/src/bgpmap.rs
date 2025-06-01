use std::collections::HashSet;

pub fn bird_route_to_graphviz(servers: &[String], responses: &[String], _target: &str) -> String {
    let mut graph = String::from("digraph {\n");
    graph.push_str("  rankdir=LR;\n");
    graph.push_str("  node [shape=box];\n");
    
    let mut nodes = HashSet::new();
    let mut edges = Vec::new();
    
    // Parse BGP routes from responses
    for (i, response) in responses.iter().enumerate() {
        if let Some(server) = servers.get(i) {
            parse_bgp_routes(response, server, &mut nodes, &mut edges);
        }
    }
    
    // Add nodes to graph
    for node in &nodes {
        graph.push_str(&format!("  \"{}\";\n", node));
    }
    
    // Add edges to graph
    for (from, to, label) in &edges {
        graph.push_str(&format!("  \"{}\" -> \"{}\" [label=\"{}\"];\n", from, to, label));
    }
    
    graph.push_str("}\n");
    graph
}

fn parse_bgp_routes(response: &str, server: &str, nodes: &mut HashSet<String>, edges: &mut Vec<(String, String, String)>) {
    // This is a simplified BGP route parser
    // In a real implementation, this would parse BIRD's route output format
    
    let lines: Vec<&str> = response.lines().collect();
    
    for line in lines {
        if line.contains("BGP.as_path") {
            // Extract AS path from BIRD output
            if let Some(as_path) = extract_as_path(line) {
                let path_elements: Vec<&str> = as_path.split_whitespace().collect();
                
                // Add nodes
                for asn in &path_elements {
                    nodes.insert(format!("AS{}", asn));
                }
                
                // Add edges
                for i in 0..path_elements.len().saturating_sub(1) {
                    let from = format!("AS{}", path_elements[i]);
                    let to = format!("AS{}", path_elements[i + 1]);
                    edges.push((from, to, server.to_string()));
                }
            }
        }
    }
}

fn extract_as_path(line: &str) -> Option<String> {
    // Extract AS path from BIRD output line
    // This is a simplified implementation
    if let Some(start) = line.find("BGP.as_path:") {
        let path_part = &line[start + 12..];
        if let Some(end) = path_part.find('\n') {
            Some(path_part[..end].trim().to_string())
        } else {
            Some(path_part.trim().to_string())
        }
    } else {
        None
    }
} 