use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

// Match encodeURIComponent in goto.js. Separators are added after encoding.
const PATH_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

pub fn encode_component(value: &str) -> String {
    utf8_percent_encode(value, PATH_COMPONENT).to_string()
}

pub fn query_url(action: &str, servers: &str, target: &str) -> String {
    if action == "whois" {
        return format!("/whois/{}", encode_component(target));
    }
    let servers = servers
        .split('+')
        .map(encode_component)
        .collect::<Vec<_>>()
        .join("+");
    let target = if action == "summary" { "" } else { target };
    format!("/{}/{}/{}", action, servers, encode_component(target))
}

// Go's html/template rejects unsafe URL schemes in configurable navigation links.
pub fn navigation_url(value: &str) -> String {
    if let Some((scheme, _)) = value.split_once(':') {
        if !scheme.contains('/')
            && !["http", "https", "mailto"]
                .iter()
                .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
        {
            return "#ZgotmplZ".to_string();
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_components_round_trip_without_becoming_url_syntax() {
        assert_eq!(
            query_url("route", "edge+core", "2001:db8::/32"),
            "/route/edge+core/2001%3Adb8%3A%3A%2F32"
        );
        assert_eq!(
            query_url("whois", "", "上海 &?#%/+"),
            "/whois/%E4%B8%8A%E6%B5%B7%20%26%3F%23%25%2F%2B"
        );
        assert_eq!(
            query_url("summary", "上海+edge", "ignored"),
            "/summary/%E4%B8%8A%E6%B5%B7+edge/"
        );
    }

    #[test]
    fn configurable_links_allow_navigation_but_not_script_urls() {
        for url in [
            "/all",
            "all-pops",
            "https://example.net",
            "//example.net",
            "mailto:ops@example.net",
        ] {
            assert_eq!(navigation_url(url), url);
        }
        for url in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:text/html,test",
        ] {
            assert_eq!(navigation_url(url), "#ZgotmplZ");
        }
    }
}
