use axum::{
    extract::Request,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::settings::Settings;
use crate::{proxy_client, whois};

#[derive(Deserialize)]
struct TgChat {
    id: i64,
}

#[derive(Deserialize)]
struct TgMessage {
    message_id: i64,
    chat: TgChat,
    text: Option<String>,
}

#[derive(Deserialize)]
struct TgWebhookRequest {
    message: Option<TgMessage>,
}

#[derive(Serialize)]
struct TgWebhookResponse {
    method: String,
    chat_id: i64,
    text: String,
    reply_to_message_id: i64,
    parse_mode: String,
}

fn telegram_is_command(message: &str, command: &str) -> bool {
    let settings = Settings::global();
    let bot_name = &settings.telegram_bot_name;
    
    if !bot_name.is_empty() {
        if message.starts_with(&format!("/{}@{} ", command, bot_name)) ||
           message == format!("/{}@{}", command, bot_name) {
            return true;
        }
    }
    
    message.starts_with(&format!("/{} ", command)) || message == format!("/{}", command)
}

fn telegram_default_post_process(s: &str) -> String {
    s.trim().to_string()
}

async fn telegram_batch_request_format(
    servers: &[String], 
    endpoint: &str, 
    command: &str,
    post_process: fn(&str) -> String
) -> String {
    let mut result = String::new();
    
    for server in servers {
        if servers.len() > 1 {
            result.push_str(&format!("{}\n", server));
        }
        
        let response = match endpoint {
            "traceroute" => proxy_client::traceroute_query(server, command).await,
            "bird" => proxy_client::bird_query(server, command).await,
            _ => Err(anyhow::anyhow!("Unknown endpoint: {}", endpoint)),
        };
        
        match response {
            Ok(res) => {
                result.push_str(&post_process(&res));
                result.push_str("\n\n");
            }
            Err(e) => {
                result.push_str(&format!("Error: {}\n\n", e));
            }
        }
    }
    
    result
}

fn extract_as_path(result: &str) -> String {
    for line in result.lines() {
        if line.contains("BGP.as_path: ") || line.contains("bgp_path: ") {
            if let Some(path) = line.split(':').nth(1) {
                return path.trim().to_string();
            }
        }
    }
    String::new()
}

async fn process_whois_command(target: &str) -> Result<String> {
    let settings = Settings::global();
    let mut target = target.to_string();
    
    // Handle dn42 specific ASN formatting
    if settings.net_specific_mode == "dn42" || settings.net_specific_mode == "dn42_generic" {
        if let Ok(target_number) = target.parse::<u64>() {
            if target_number < 10000 {
                target = format!("AS{}", target_number + 4242420000);
            } else {
                target = format!("AS{}", target);
            }
        }
    }
    
    let temp_result = whois::query(&target).await?;
    
    // Apply network-specific filters
    let result = match settings.net_specific_mode.as_str() {
        "dn42" => dn42_whois_filter(&temp_result),
        "dn42_shorten" | "shorten" => shorten_whois_filter(&temp_result),
        _ => temp_result,
    };
    
    Ok(result)
}

// Simplified whois filters (you may want to implement these based on Go version)
fn dn42_whois_filter(result: &str) -> String {
    // Simplified implementation - filter relevant dn42 information
    result.lines()
        .filter(|line| {
            line.contains("aut-num:") ||
            line.contains("as-name:") ||
            line.contains("descr:") ||
            line.contains("admin-c:") ||
            line.contains("tech-c:") ||
            line.contains("mnt-by:")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn shorten_whois_filter(result: &str) -> String {
    // Simplified implementation - show only essential information
    result.lines()
        .take(20) // Limit to first 20 lines
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn telegram_webhook(request: Request) -> impl IntoResponse {
    // Extract the path to get servers list
    let path = request.uri().path().to_string();
    let servers_path = if path.starts_with("/telegram/") {
        &path[10..] // Remove "/telegram/" prefix
    } else {
        ""
    };
    
    // Parse JSON body
    let body_bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return (StatusCode::BAD_REQUEST, "Failed to read body").into_response(),
    };
    
    let webhook_request: TgWebhookRequest = match serde_json::from_slice(&body_bytes) {
        Ok(req) => req,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response(),
    };
    
    // Extract message
    let message = match webhook_request.message {
        Some(msg) => msg,
        None => return StatusCode::OK.into_response(),
    };
    
    let text = match message.text {
        Some(text) => text,
        None => return StatusCode::OK.into_response(),
    };
    
    // Only respond to commands (starting with /)
    if !text.starts_with('/') {
        return StatusCode::OK.into_response();
    }
    
    let settings = Settings::global();
    
    // Select servers based on webhook URL
    let servers = if servers_path.is_empty() {
        settings.servers.clone()
    } else {
        servers_path.split('+').map(|s| s.to_string()).collect()
    };
    
    // Parse target from command
    let target = if text.contains(' ') {
        text.split_whitespace().skip(1).collect::<Vec<_>>().join(" ").trim().to_string()
    } else {
        String::new()
    };
    
    // Execute command
    let command_result = if telegram_is_command(&text, "trace") {
        telegram_batch_request_format(&servers, "traceroute", &target, telegram_default_post_process).await
        
    } else if telegram_is_command(&text, "route") {
        let command = format!("show route for {} primary", target);
        telegram_batch_request_format(&servers, "bird", &command, telegram_default_post_process).await
        
    } else if telegram_is_command(&text, "path") {
        let command = format!("show route for {} all primary", target);
        telegram_batch_request_format(&servers, "bird", &command, |result| extract_as_path(result)).await
        
    } else if telegram_is_command(&text, "whois") {
        match process_whois_command(&target).await {
            Ok(result) => result,
            Err(e) => format!("Error: {}", e),
        }
        
    } else if telegram_is_command(&text, "help") {
        "/path <IP>\n/route <IP>\n/trace <IP>\n/whois <Target>".to_string()
        
    } else {
        return StatusCode::OK.into_response();
    };
    
    let command_result = command_result.trim();
    let command_result = if command_result.is_empty() {
        "empty result"
    } else {
        command_result
    };
    
    // Limit response length to Telegram's maximum
    let command_result = if command_result.len() > 4096 {
        &command_result[..4096]
    } else {
        command_result
    };
    
    // Create JSON response
    let response = TgWebhookResponse {
        method: "sendMessage".to_string(),
        chat_id: message.chat.id,
        text: format!("```\n{}\n```", command_result),
        reply_to_message_id: message.message_id,
        parse_mode: "Markdown".to_string(),
    };
    
    match serde_json::to_string(&response) {
        Ok(json) => (
            StatusCode::OK,
            [("Content-Type", "application/json")],
            json
        ).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize response").into_response(),
    }
} 