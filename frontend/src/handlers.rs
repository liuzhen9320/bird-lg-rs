use axum::{
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use crate::settings::Settings;
use crate::templates::{PageContext, BirdContext, WhoisContext, BgpmapContext};
use crate::{proxy_client, whois, bgpmap, templates};
use base64::{Engine as _, engine::general_purpose};

// Redirect to summary page
pub async fn redirect_to_summary() -> impl IntoResponse {
    let settings = Settings::global();
    let all_servers = settings.all_servers_string();
    Redirect::permanent(&format!("/summary/{}", all_servers))
}

// Bird summary handler
pub async fn bird_summary(Path(servers): Path<String>) -> Result<impl IntoResponse, Response> {
    handle_bird_command(servers, "summary", "show protocols".to_string()).await
}

// Bird detail handler
pub async fn bird_detail(Path((servers, protocol)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show protocols all {}", protocol);
    handle_bird_command(servers, "detail", command).await
}

// Bird route handler
pub async fn bird_route(Path((servers, route)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route for {}", route);
    handle_bird_command(servers, "route", command).await
}

// Bird route all handler
pub async fn bird_route_all(Path((servers, route)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route for {} all", route);
    handle_bird_command(servers, "route_all", command).await
}

// Bird route where handler
pub async fn bird_route_where(Path((servers, prefix)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route where net ~ [ {} ]", prefix);
    handle_bird_command(servers, "route_where", command).await
}

// Bird route where all handler
pub async fn bird_route_where_all(Path((servers, prefix)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route where net ~ [ {} ] all", prefix);
    handle_bird_command(servers, "route_where_all", command).await
}

// Bird route from protocol handler
pub async fn bird_route_from_protocol(Path((servers, protocol)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route protocol {}", protocol);
    handle_bird_command(servers, "route_from_protocol", command).await
}

// Bird route from protocol all handler
pub async fn bird_route_from_protocol_all(Path((servers, protocol)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route protocol {} all", protocol);
    handle_bird_command(servers, "route_from_protocol_all", command).await
}

// Bird route filtered from protocol handler
pub async fn bird_route_filtered_from_protocol(Path((servers, protocol)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route filtered protocol {}", protocol);
    handle_bird_command(servers, "route_filtered_from_protocol", command).await
}

// Bird route filtered from protocol all handler
pub async fn bird_route_filtered_from_protocol_all(Path((servers, protocol)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route filtered protocol {} all", protocol);
    handle_bird_command(servers, "route_filtered_from_protocol_all", command).await
}

// Bird route from origin handler
pub async fn bird_route_from_origin(Path((servers, asn)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route where bgp_path.last = {}", asn);
    handle_bird_command(servers, "route_from_origin", command).await
}

// Bird route from origin all handler
pub async fn bird_route_from_origin_all(Path((servers, asn)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route where bgp_path.last = {} all", asn);
    handle_bird_command(servers, "route_from_origin_all", command).await
}

// Bird generic command handler
pub async fn bird_generic(Path((servers, command)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show {}", command);
    handle_bird_command(servers, "generic", command).await
}

// BGP Map handlers
pub async fn bird_route_bgpmap(Path((servers, route)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route for {} all", route);
    handle_bgpmap_command(servers, command, route).await
}

pub async fn bird_route_where_bgpmap(Path((servers, prefix)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let command = format!("show route where net ~ [ {} ] all", prefix);
    handle_bgpmap_command(servers, command, prefix).await
}

// Traceroute handler
pub async fn traceroute(Path((servers, target)): Path<(String, String)>) -> Result<impl IntoResponse, Response> {
    let server_list: Vec<String> = servers.split('+').map(|s| s.to_string()).collect();
    let settings = Settings::global();
    
    let mut content = String::new();
    
    for server in &server_list {
        let display_name = settings.get_server_display_name(server);
        
        match proxy_client::traceroute_query(server, &target).await {
            Ok(result) => {
                let bird_context = BirdContext {
                    server_name: display_name,
                    target: target.clone(),
                    result: format!("<pre>{}</pre>", html_escape::encode_text(&result)),
                };
                
                match templates::render_bird(&bird_context) {
                    Ok(rendered) => content.push_str(&rendered),
                    Err(e) => content.push_str(&format!("<p>Template error: {}</p>", e)),
                }
            }
            Err(e) => {
                content.push_str(&format!("<h2>{}: traceroute {}</h2><p>Error: {}</p>", display_name, target, e));
            }
        }
    }
    
    let page_context = build_page_context("traceroute", &servers, &target, &content);
    
    match templates::render_page(&page_context) {
        Ok(html) => Ok(Html(html)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response()),
    }
}

// Whois handler
pub async fn whois(Path(target): Path<String>) -> Result<impl IntoResponse, Response> {
    match whois::query(&target).await {
        Ok(result) => {
            let whois_context = WhoisContext {
                target: target.clone(),
                result: format!("<pre>{}</pre>", html_escape::encode_text(&result)),
            };
            
            let content = match templates::render_whois(&whois_context) {
                Ok(rendered) => rendered,
                Err(e) => format!("<p>Template error: {}</p>", e),
            };
            
            let page_context = build_whois_page_context(&target, &content);
            
            match templates::render_page(&page_context) {
                Ok(html) => Ok(Html(html)),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response()),
            }
        }
        Err(e) => {
            let content = format!("<h2>whois {}</h2><p>Error: {}</p>", target, e);
            let page_context = build_whois_page_context(&target, &content);
            
            match templates::render_page(&page_context) {
                Ok(html) => Ok(Html(html)),
                Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response()),
            }
        }
    }
}

// Helper function to handle bird commands
async fn handle_bird_command(servers: String, option: &str, command: String) -> Result<impl IntoResponse, Response> {
    let server_list: Vec<String> = servers.split('+').map(|s| s.to_string()).collect();
    let settings = Settings::global();
    
    let mut content = String::new();
    
    for server in &server_list {
        let display_name = settings.get_server_display_name(server);
        
        match proxy_client::bird_query(server, &command).await {
            Ok(result) => {
                let formatted_result = if option == "summary" && result.starts_with("Name") {
                    format_summary_table(&result, server)
                } else {
                    format!("<pre>{}</pre>", html_escape::encode_text(&result))
                };
                
                let bird_context = BirdContext {
                    server_name: display_name,
                    target: command.clone(),
                    result: formatted_result,
                };
                
                match templates::render_bird(&bird_context) {
                    Ok(rendered) => content.push_str(&rendered),
                    Err(e) => content.push_str(&format!("<p>Template error: {}</p>", e)),
                }
            }
            Err(e) => {
                content.push_str(&format!("<h2>{}: {}</h2><p>Error: {}</p>", display_name, command, e));
            }
        }
    }
    
    let page_context = build_page_context(option, &servers, &command, &content);
    
    match templates::render_page(&page_context) {
        Ok(html) => Ok(Html(html)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response()),
    }
}

// Helper function to handle BGP map commands
async fn handle_bgpmap_command(servers: String, command: String, target: String) -> Result<impl IntoResponse, Response> {
    let server_list: Vec<String> = servers.split('+').map(|s| s.to_string()).collect();
    
    let mut responses = Vec::new();
    for server in &server_list {
        match proxy_client::bird_query(server, &command).await {
            Ok(result) => responses.push(result),
            Err(e) => responses.push(format!("Error from {}: {}", server, e)),
        }
    }
    
    let dot_graph = bgpmap::bird_route_to_graphviz(&server_list, &responses, &target);
    let encoded_graph = general_purpose::STANDARD.encode(dot_graph);
    
    let bgpmap_context = BgpmapContext {
        target: target.clone(),
        result: encoded_graph,
    };
    
    let content = match templates::render_bgpmap(&bgpmap_context) {
        Ok(rendered) => rendered,
        Err(e) => format!("<p>Template error: {}</p>", e),
    };
    
    let page_context = build_page_context("bgpmap", &servers, &target, &content);
    
    match templates::render_page(&page_context) {
        Ok(html) => Ok(Html(html)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response()),
    }
}

// Helper function to build page context
fn build_page_context(option: &str, servers: &str, command: &str, content: &str) -> PageContext {
    let settings = Settings::global();
    
    PageContext {
        title: format!("{} - {} {}", settings.title_brand, option, command),
        brand: settings.navbar_brand.clone(),
        brand_url: settings.navbar_brand_url.clone(),
        all_server_title: settings.navbar_all_server.clone(),
        all_servers_url: settings.all_servers_string(),
        all_servers_link_active: servers == settings.all_servers_string(),
        servers: settings.servers.clone(),
        servers_display: settings.servers_display.clone(),
        url_option: option.to_string(),
        url_server: servers.to_string(),
        url_command: command.to_string(),
        options: get_options(),
        content: content.to_string(),
    }
}

// Helper function to build whois page context
fn build_whois_page_context(target: &str, content: &str) -> PageContext {
    let settings = Settings::global();
    
    PageContext {
        title: format!("{} - whois {}", settings.title_brand, target),
        brand: settings.navbar_brand.clone(),
        brand_url: settings.navbar_brand_url.clone(),
        all_server_title: settings.navbar_all_server.clone(),
        all_servers_url: settings.all_servers_string(),
        all_servers_link_active: false,
        servers: settings.servers.clone(),
        servers_display: settings.servers_display.clone(),
        url_option: "whois".to_string(),
        url_server: settings.all_servers_string(),
        url_command: target.to_string(),
        options: get_options(),
        content: content.to_string(),
    }
}

// Get available options for the dropdown
fn get_options() -> Vec<(String, String)> {
    vec![
        ("summary".to_string(), "Summary".to_string()),
        ("detail".to_string(), "Detail".to_string()),
        ("route".to_string(), "Route".to_string()),
        ("route_all".to_string(), "Route (all)".to_string()),
        ("route_where".to_string(), "Route where".to_string()),
        ("route_where_all".to_string(), "Route where (all)".to_string()),
        ("route_bgpmap".to_string(), "Route BGP map".to_string()),
        ("route_where_bgpmap".to_string(), "Route where BGP map".to_string()),
        ("traceroute".to_string(), "Traceroute".to_string()),
        ("whois".to_string(), "Whois".to_string()),
        ("generic".to_string(), "Generic".to_string()),
    ]
}

// Format summary table (simplified version)
fn format_summary_table(result: &str, _server: &str) -> String {
    // This is a simplified version - the full implementation would parse
    // the BIRD protocol output and create an HTML table
    format!("<pre>{}</pre>", html_escape::encode_text(result))
} 