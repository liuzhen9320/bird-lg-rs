use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    Bird,
    Bgpmap,
    Traceroute,
    Whois,
}

#[derive(Serialize)]
pub struct Query {
    pub action: &'static str,
    pub label: &'static str,
    #[serde(skip)]
    pub kind: QueryKind,
    #[serde(skip)]
    command: &'static str,
}

impl Query {
    pub fn command(&self, target: &str) -> String {
        self.command.replace("{}", target).trim().to_string()
    }
}

// bird-lg-go 4f787e5 frontend/render.go and webserver.go, plus the two Rust
// primary-only entries. Ordered by action, like Go's template map iteration.
pub static QUERIES: &[Query] = &[
    Query {
        action: "detail",
        label: "show protocols all ...",
        kind: QueryKind::Bird,
        command: "show protocols all '{}'",
    },
    Query {
        action: "generic",
        label: "show ...",
        kind: QueryKind::Bird,
        command: "show {}",
    },
    Query {
        action: "route",
        label: "show route for ...",
        kind: QueryKind::Bird,
        command: "show route for {}",
    },
    Query {
        action: "route_all",
        label: "show route for ... all",
        kind: QueryKind::Bird,
        command: "show route for {} all",
    },
    Query {
        action: "route_bgpmap",
        label: "show route for ... (bgpmap)",
        kind: QueryKind::Bgpmap,
        command: "show route for {} all",
    },
    Query {
        action: "route_filtered_from_protocol",
        label: "show route filtered protocol ...",
        kind: QueryKind::Bird,
        command: "show route filtered protocol '{}'",
    },
    Query {
        action: "route_filtered_from_protocol_all",
        label: "show route filtered protocol ... all",
        kind: QueryKind::Bird,
        command: "show route filtered protocol '{}' all",
    },
    Query {
        action: "route_from_origin",
        label: "show route where bgp_path.last = ...",
        kind: QueryKind::Bird,
        command: "show route where bgp_path.last = {}",
    },
    Query {
        action: "route_from_origin_all",
        label: "show route where bgp_path.last = ... all",
        kind: QueryKind::Bird,
        command: "show route where bgp_path.last = {} all",
    },
    Query {
        action: "route_from_origin_all_primary",
        label: "show route where bgp_path.last = ... all primary",
        kind: QueryKind::Bird,
        command: "show route where bgp_path.last = {} all primary",
    },
    Query {
        action: "route_from_origin_primary",
        label: "show route where bgp_path.last = ... primary",
        kind: QueryKind::Bird,
        command: "show route where bgp_path.last = {} primary",
    },
    Query {
        action: "route_from_protocol",
        label: "show route protocol ...",
        kind: QueryKind::Bird,
        command: "show route protocol '{}'",
    },
    Query {
        action: "route_from_protocol_all",
        label: "show route protocol ... all",
        kind: QueryKind::Bird,
        command: "show route protocol '{}' all",
    },
    Query {
        action: "route_from_protocol_all_primary",
        label: "show route protocol ... all primary",
        kind: QueryKind::Bird,
        command: "show route protocol '{}' all primary",
    },
    Query {
        action: "route_from_protocol_primary",
        label: "show route protocol ... primary",
        kind: QueryKind::Bird,
        command: "show route protocol '{}' primary",
    },
    Query {
        action: "route_generic",
        label: "show route ...",
        kind: QueryKind::Bird,
        command: "show route {}",
    },
    Query {
        action: "route_where",
        label: "show route where net ~ [ ... ]",
        kind: QueryKind::Bird,
        command: "show route where net ~ [ {} ]",
    },
    Query {
        action: "route_where_all",
        label: "show route where net ~ [ ... ] all",
        kind: QueryKind::Bird,
        command: "show route where net ~ [ {} ] all",
    },
    Query {
        action: "route_where_bgpmap",
        label: "show route where net ~ [ ... ] (bgpmap)",
        kind: QueryKind::Bgpmap,
        command: "show route where net ~ [ {} ] all",
    },
    Query {
        action: "summary",
        label: "show protocols",
        kind: QueryKind::Bird,
        command: "show protocols",
    },
    Query {
        action: "traceroute",
        label: "traceroute ...",
        kind: QueryKind::Traceroute,
        command: "{}",
    },
    Query {
        action: "whois",
        label: "whois ...",
        kind: QueryKind::Whois,
        command: "{}",
    },
];
