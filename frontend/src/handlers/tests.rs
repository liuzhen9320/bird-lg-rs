use super::*;
use crate::test_args;

fn settings(spec: &str) -> Settings {
    Settings::from_args(test_args(&["bird-lg-rs", &format!("--servers={spec}")])).unwrap()
}

fn query_named(action: &str) -> &'static Query {
    QUERIES.iter().find(|query| query.action == action).unwrap()
}

fn page(settings: &Settings, action: &str, servers: &str, target: &str) -> (PageContext, String) {
    templates::init_for_test();
    let query = query_named(action);
    let servers = if query.kind == QueryKind::Whois {
        settings.servers.clone()
    } else {
        settings
            .resolve_servers_from_display_names(servers)
            .unwrap()
    };
    let context = build_page_context(settings, query, &servers, target, &query.command(target));
    let html = templates::render_page(&context, &[]).unwrap();
    (
        context,
        html_escape::decode_html_entities(&html).into_owned(),
    )
}

#[test]
fn node_navigation_preserves_the_query_and_normalizes_old_aliases() {
    let settings = settings("Shanghai<edge>,Core<core>");
    let (context, html) = page(&settings, "route", "Shanghai", "2001:db8::/32");
    assert_eq!(context.url_server, "edge");
    assert_eq!(context.url_command, "2001:db8::/32");
    assert_eq!(
        context.title,
        "Bird-lg Rust - bird show route for 2001:db8::/32"
    );
    assert_eq!(context.brand, "Bird-lg Rust");
    assert!(html.contains("href=\"/route/core/2001%3Adb8%3A%3A%2F32\""));
    assert!(html.contains("href=\"/route/edge+core/2001%3Adb8%3A%3A%2F32\""));
    assert!(html.contains("value=\"2001:db8::/32\""));
    assert!(html.contains("value=\"route\" selected>show route for ...</option>"));
    assert!(context.servers[0].active);
    assert!(!context.servers[1].active);
    assert!(!context.all_servers.active);
    assert!(
        page(&settings, "summary", "Shanghai+Core", "")
            .0
            .all_servers
            .active
    );
}

#[test]
fn single_node_custom_all_link_and_whois_follow_upstream_navigation() {
    let single = settings("Shanghai<edge>");
    let (context, _) = page(&single, "route", "edge", "1.1.1.1");
    assert_eq!(context.servers[0].href, "/");
    assert!(context.servers[0].active);

    let mut multiple = settings("Shanghai<edge>,Core<core>");
    let (context, html) = page(&multiple, "whois", "", "AS64500");
    assert_eq!(context.url_option, "whois");
    assert_eq!(context.url_server, "edge+core");
    assert_eq!(context.url_command, "AS64500");
    assert_eq!(context.servers[0].href, "/summary/edge/");
    assert_eq!(context.all_servers.href, "/summary/edge+core/");
    assert!(!context.all_servers.active);
    assert!(html.contains("value=\"whois\" selected>whois ...</option>"));
    for link in ["/all-pops", "https://example.net/lg", "all-pops"] {
        multiple.navbar_all_url = link.into();
        let (context, _) = page(&multiple, "route", "edge", "1.1.1.1");
        assert_eq!(context.all_servers.href, link);
        assert!(context.all_servers.active);
    }
}

#[test]
fn both_maps_keep_their_action_target_and_command_title() {
    let settings = settings("edge,core");
    for (action, command) in [
        ("route_bgpmap", "show route for 192.0.2.0/24 all"),
        (
            "route_where_bgpmap",
            "show route where net ~ [ 192.0.2.0/24 ] all",
        ),
    ] {
        let (context, html) = page(&settings, action, "edge", "192.0.2.0/24");
        assert_eq!(context.url_command, "192.0.2.0/24");
        assert_eq!(context.title, format!("Bird-lg Rust - bird {command}"));
        assert!(html.contains(&format!("value=\"{action}\" selected>")));
        assert_eq!(
            context.servers[1].href,
            format!("/{action}/core/192.0.2.0%2F24")
        );
    }
}

#[test]
fn all_menu_entries_render_in_upstream_order_with_primary_extensions() {
    let (_, html) = page(&settings("edge"), "summary", "edge", "");
    assert_eq!(html.matches("<option ").count(), 22);
    assert!(QUERIES
        .windows(2)
        .all(|pair| pair[0].action < pair[1].action));
    assert!(html.contains("value=\"summary\" selected>show protocols</option>"));
    assert!(html.contains(
        "value=\"route_from_protocol_primary\">show route protocol ... primary</option>"
    ));
    assert!(html.contains(
        "value=\"route_from_origin_primary\">show route where bgp_path.last = ... primary</option>"
    ));
    assert_eq!(
        query_named("detail").command("peer-transit"),
        "show protocols all 'peer-transit'"
    );
    assert_eq!(
        query_named("route_from_protocol_primary").command("peer-transit"),
        "show route protocol 'peer-transit' primary"
    );
    assert_eq!(
        query_named("route_from_origin_primary").command("64500"),
        "show route where bgp_path.last = 64500 primary"
    );
}

#[test]
fn summary_and_generic_show_protocols_render_a_complete_page() {
    templates::init_for_test();
    let settings = settings("Display<edge>");
    let data = include_str!("../../tests/fixtures/summary.txt");
    for action in ["summary", "generic"] {
        let target = if action == "generic" { "protocols" } else { "" };
        let context = BirdContext {
            server_name: "Display".into(),
            target: "show protocols".into(),
            result: data.into(),
        };
        let content = render_bird_result(&settings, &context, "edge").unwrap();
        let page_context = page(&settings, action, "edge", target).0;
        let html = templates::render_page(&page_context, &[content]).unwrap();
        let decoded = html_escape::decode_html_entities(&html);
        assert!(decoded.contains("<h2>Display: show protocols</h2>"));
        assert!(decoded
            .contains("<table class=\"table table-striped table-bordered table-sm sortable\">"));
        assert!(decoded.contains("href=\"/detail/edge/int_babel\""));
        assert!(!decoded.contains("/detail/Display/"));
        assert_eq!(decoded.matches("<th scope=\"col\">").count(), 6);
        assert_eq!(decoded.matches("<tr class=\"table-success\">").count(), 7);
    }
    let invalid = BirdContext {
        server_name: "Display".into(),
        target: "show protocols".into(),
        result: "Name <bad header>".into(),
    };
    let html = render_bird_result(&settings, &invalid, "edge").unwrap();
    assert!(html.as_str().contains("<pre>Name &lt;bad header&gt;</pre>"));
}

#[test]
fn output_links_preserve_text_without_interpreting_backend_html() {
    templates::init_for_test();
    let text = "BGP.as_path: 64500 64501\nNeighbor AS: 64502\nLocal AS: 64503\nbgp_path: 64504\n[AS64505i] 192.0.2.1 2001:db8::1 example.net\n<script>alert(1)</script><img src=x onerror=alert(1)> & \"quoted\"";
    let bird = templates::render_bird(&BirdContext {
        server_name: "edge".into(),
        target: "show route".into(),
        result: text.into(),
    })
    .unwrap();
    let whois = templates::render_whois(&WhoisContext {
        target: "AS64500".into(),
        result: text.into(),
    })
    .unwrap();
    for rendered in [bird, whois] {
        let html = rendered.as_str();
        let decoded = html_escape::decode_html_entities(html);
        for target in [
            "AS64500",
            "AS64501",
            "AS64502",
            "AS64503",
            "AS64504",
            "AS64505",
            "192.0.2.1",
            "2001%3Adb8%3A%3A1",
            "example.net",
        ] {
            assert!(
                decoded.contains(&format!("href=\"/whois/{target}\" class=\"whois\"")),
                "missing {target}: {decoded}"
            );
        }
        assert_eq!(html.matches("<a ").count(), 9);
        assert_eq!(html.matches("</a>").count(), 9);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&lt;img"));
        assert!(html.contains("&amp;"));
    }
}

#[test]
fn query_failure_templates_escape_untrusted_values_and_preserve_lines() {
    templates::init_for_test();
    let payload = "\"><script>alert(1)</script><img src=x onerror=alert(1)>\nsecond line";
    for heading in [
        format!("edge: {payload}"),
        format!("whois {payload}"),
        format!("edge: show route for {payload}"),
    ] {
        let rendered = render_query_error(heading, payload).unwrap();
        assert!(!rendered.as_str().contains("<script>"));
        assert!(!rendered.as_str().contains("<img"));
        assert!(rendered.as_str().contains("&lt;script&gt;"));
        assert!(rendered.as_str().contains("\nsecond line</pre>"));
        assert!(!rendered.as_str().contains("<p>"));
    }
}
