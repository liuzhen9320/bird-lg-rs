use axum::{
    extract::Path,
    response::{IntoResponse, Json},
};
use serde_json::json;
use crate::{proxy_client, whois, settings::Settings};

pub async fn bird_api(Path((servers, command)): Path<(String, String)>) -> impl IntoResponse {
    let settings = Settings::global();
    let server_list = settings.resolve_servers_from_display_names(&servers);
    let mut results = Vec::new();
    
    for server in &server_list {
        match proxy_client::bird_query(server, &command).await {
            Ok(result) => {
                results.push(json!({
                    "server": server,
                    "result": result,
                    "error": null
                }));
            }
            Err(e) => {
                results.push(json!({
                    "server": server,
                    "result": null,
                    "error": e.to_string()
                }));
            }
        }
    }
    
    Json(json!({
        "servers": server_list,
        "command": command,
        "results": results
    }))
}

pub async fn traceroute_api(Path((servers, target)): Path<(String, String)>) -> impl IntoResponse {
    let settings = Settings::global();
    let server_list = settings.resolve_servers_from_display_names(&servers);
    let mut results = Vec::new();
    
    for server in &server_list {
        match proxy_client::traceroute_query(server, &target).await {
            Ok(result) => {
                results.push(json!({
                    "server": server,
                    "result": result,
                    "error": null
                }));
            }
            Err(e) => {
                results.push(json!({
                    "server": server,
                    "result": null,
                    "error": e.to_string()
                }));
            }
        }
    }
    
    Json(json!({
        "servers": server_list,
        "target": target,
        "results": results
    }))
}

pub async fn whois_api(Path(target): Path<String>) -> impl IntoResponse {
    match whois::query(&target).await {
        Ok(result) => {
            Json(json!({
                "target": target,
                "result": result,
                "error": null
            }))
        }
        Err(e) => {
            Json(json!({
                "target": target,
                "result": null,
                "error": e.to_string()
            }))
        }
    }
} 