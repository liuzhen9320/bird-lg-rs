use crate::queries::{Query, QueryKind, QUERIES};
use crate::settings::Settings;
use crate::templates::{
    BgpmapContext, BirdContext, NavigationLink, PageContext, QueryErrorContext, TrustedHtml,
    WhoisContext,
};
use crate::urls::{navigation_url, query_url};
use crate::{bgpmap, proxy_client, summary_parser, templates, whois};
use axum::{
    http::{StatusCode, Uri},
    response::{Html, Redirect},
};
use base64::{engine::general_purpose, Engine as _};
use serde::Deserialize;

pub(crate) type HandlerError = (StatusCode, String);
pub(crate) type HandlerResult<T> = Result<T, HandlerError>;

#[derive(Deserialize)]
pub(crate) struct PageRequest {
    #[serde(default)]
    pub servers: String,
    #[serde(default)]
    pub target: String,
}

pub(crate) async fn redirect_to_summary() -> Redirect {
    Redirect::temporary(&query_url(
        "summary",
        &Settings::global().all_server_ids(),
        "",
    ))
}

pub(crate) async fn query(
    query: &'static Query,
    mut request: PageRequest,
    uri: Uri,
) -> HandlerResult<Html<String>> {
    // Keep the existing trailing-slash aliases, but preserve a slash encoded as
    // part of the target (%2F). Axum has already decoded path parameters once.
    if uri.path().ends_with('/') && request.target.ends_with('/') {
        request.target.pop();
    }
    let settings = Settings::global();
    let command = query.command(&request.target);
    let servers = if query.kind == QueryKind::Whois {
        settings.servers.clone()
    } else {
        settings
            .resolve_servers_from_display_names(&request.servers)
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?
    };
    if servers.len() > settings.servers.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid request: too many servers specified".into(),
        ));
    }
    proxy_client::validate_servers(&servers)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;

    let content = match query.kind {
        QueryKind::Whois => vec![match whois::query(&request.target).await {
            Ok(result) => templates::render_whois(&WhoisContext {
                target: request.target.clone(),
                result,
            }),
            Err(error) => {
                render_query_error(format!("whois {}", request.target), &error.to_string())
            }
        }
        .map_err(template_error_response)?],
        QueryKind::Bgpmap => {
            let mut responses = Vec::new();
            for server in &servers {
                match proxy_client::bird_query(server, &command).await {
                    Ok(result) => responses.push(result),
                    Err(error) => responses.push(format!("Error from {}: {}", server, error)),
                }
            }
            let graph = bgpmap::bird_route_to_graphviz(&servers, &responses, &request.target);
            vec![templates::render_bgpmap(&BgpmapContext {
                target: command.clone(),
                result: general_purpose::STANDARD.encode(graph),
            })
            .map_err(template_error_response)?]
        }
        QueryKind::Bird | QueryKind::Traceroute => {
            let mut content = Vec::new();
            for server in &servers {
                let display_name = settings.get_server_display_name(server);
                let response = if query.kind == QueryKind::Traceroute {
                    proxy_client::traceroute_query(server, &command).await
                } else {
                    proxy_client::bird_query(server, &command).await
                };
                let rendered = match response {
                    Ok(result) => {
                        let context = BirdContext {
                            server_name: display_name,
                            target: command.clone(),
                            result,
                        };
                        if query.kind == QueryKind::Bird {
                            render_bird_result(settings, &context, settings.get_server_id(server))
                        } else {
                            templates::render_bird(&context)
                        }
                    }
                    Err(error) => render_query_error(
                        format!("{}: {}", display_name, command),
                        &error.to_string(),
                    ),
                }
                .map_err(template_error_response)?;
                content.push(rendered);
            }
            content
        }
    };
    let context = build_page_context(settings, query, &servers, &request.target, &command);
    templates::render_page(&context, &content)
        .map(Html)
        .map_err(template_error_response)
}

fn build_page_context(
    settings: &Settings,
    query: &'static Query,
    servers: &[String],
    target: &str,
    command: &str,
) -> PageContext {
    let selected_ids = servers
        .iter()
        .map(|server| settings.get_server_id(server))
        .collect::<Vec<_>>()
        .join("+");
    let all_ids = settings.all_server_ids();
    let is_whois = query.kind == QueryKind::Whois;
    let (nav_action, nav_target) = if is_whois {
        ("summary", "")
    } else {
        (query.action, target)
    };
    let custom_all = settings.navbar_all_url != "all";
    let endpoint = match query.kind {
        QueryKind::Bird | QueryKind::Bgpmap => "bird",
        QueryKind::Traceroute => "traceroute",
        QueryKind::Whois => "whois",
    };

    PageContext {
        title: format!("{} - {} {}", settings.title_brand, endpoint, command),
        brand: settings.navbar_brand.clone(),
        brand_url: navigation_url(&settings.navbar_brand_url),
        all_servers: NavigationLink {
            label: settings.navbar_all_server.clone(),
            href: if custom_all {
                navigation_url(&settings.navbar_all_url)
            } else {
                query_url(nav_action, &all_ids, nav_target)
            },
            active: custom_all || (!is_whois && selected_ids.eq_ignore_ascii_case(&all_ids)),
        },
        servers: settings
            .server_ids
            .iter()
            .zip(&settings.servers_display)
            .map(|(id, label)| NavigationLink {
                label: label.clone(),
                href: if settings.server_ids.len() == 1 {
                    "/".into()
                } else {
                    query_url(nav_action, id, nav_target)
                },
                active: &selected_ids == id,
            })
            .collect(),
        url_option: query.action.to_string(),
        url_server: selected_ids,
        url_command: target.to_string(),
        options: QUERIES,
    }
}

fn render_bird_result(
    settings: &Settings,
    context: &BirdContext,
    server_id: &str,
) -> anyhow::Result<TrustedHtml> {
    if context.target == "show protocols"
        && context
            .result
            .get(..4)
            .is_some_and(|header| header.eq_ignore_ascii_case("name"))
        && context.result.len() > 4
    {
        return match summary_parser::parse_summary(
            &context.result,
            server_id.to_string(),
            &settings.protocol_filter,
            settings.name_filter.as_ref(),
        ) {
            Ok(summary_context) => {
                let summary = templates::render_summary(&summary_context)?;
                templates::render_bird_with_html(context, &summary)
            }
            Err(_) => templates::render_bird_plain(context),
        };
    }
    templates::render_bird(context)
}

fn render_query_error(heading: String, error: &str) -> anyhow::Result<TrustedHtml> {
    templates::render_query_error(&QueryErrorContext {
        heading,
        error: error.to_string(),
    })
}

fn template_error_response(error: anyhow::Error) -> HandlerError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Template error: {}", error),
    )
}

#[cfg(test)]
mod tests;
