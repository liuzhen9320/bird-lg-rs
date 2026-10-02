use crate::{formatter, queries::Query};
use anyhow::{anyhow, Result};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::sync::OnceLock;
use tera::{Context, Tera};

#[derive(RustEmbed)]
#[folder = "assets/templates"]
struct Templates;

#[derive(RustEmbed)]
#[folder = "assets/static"]
pub struct StaticAssets;

static TEMPLATES: OnceLock<Tera> = OnceLock::new();

#[derive(Serialize)]
pub struct PageContext {
    pub title: String,
    pub brand: String,
    pub brand_url: String,
    pub all_servers: NavigationLink,
    pub servers: Vec<NavigationLink>,
    pub url_option: String,
    pub url_server: String,
    pub url_command: String,
    pub options: &'static [Query],
}

#[derive(Serialize)]
pub struct NavigationLink {
    pub label: String,
    pub href: String,
    pub active: bool,
}

#[derive(Serialize)]
pub struct BirdContext {
    pub server_name: String,
    pub target: String,
    pub result: String,
}

#[derive(Serialize)]
pub struct WhoisContext {
    pub target: String,
    pub result: String,
}

#[derive(Serialize)]
pub struct BgpmapContext {
    pub target: String,
    pub result: String,
}

#[derive(Serialize)]
pub struct QueryErrorContext {
    pub heading: String,
    pub error: String,
}

pub(crate) struct TrustedHtml(String);

impl TrustedHtml {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize)]
pub struct SummaryContext {
    pub server_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<SummaryRowData>,
}

#[derive(Serialize)]
pub struct SummaryRowData {
    pub detail_url: String,
    pub name: String,
    pub proto: String,
    pub table: String,
    pub state: String,
    pub mapped_state: String,
    pub since: String,
    pub info: String,
}

pub fn init() -> Result<()> {
    let mut tera = Tera::default();

    // Load embedded templates
    for file in Templates::iter() {
        let content =
            Templates::get(&file).ok_or_else(|| anyhow!("Template {} not found", file))?;
        let content_str = std::str::from_utf8(content.data.as_ref())?;
        tera.add_raw_template(&file, content_str)?;
    }

    tera.autoescape_on(vec!["html"]);

    TEMPLATES
        .set(tera)
        .map_err(|_| anyhow::anyhow!("Templates already initialized"))?;
    Ok(())
}

pub fn get_templates() -> &'static Tera {
    TEMPLATES.get().expect("Templates not initialized")
}

#[cfg(test)]
pub fn init_for_test() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| init().unwrap());
}

fn render_trusted(template: &str, context: &Context) -> Result<TrustedHtml> {
    let tera = get_templates();
    Ok(TrustedHtml(tera.render(template, context)?))
}

pub fn render_page(context: &PageContext, content: &[TrustedHtml]) -> Result<String> {
    let tera = get_templates();
    let mut context = Context::from_serialize(context)?;
    let content = content.iter().map(TrustedHtml::as_str).collect::<String>();
    context.insert("trusted_content", &content);
    let rendered = tera.render("page.html", &context)?;
    Ok(rendered)
}

pub fn render_bird(context: &BirdContext) -> Result<TrustedHtml> {
    let result = render_formatted_result(&context.result)?;
    render_bird_with_html(context, &result)
}

pub fn render_bird_plain(context: &BirdContext) -> Result<TrustedHtml> {
    render_trusted("bird.html", &Context::from_serialize(context)?)
}

pub fn render_bird_with_html(context: &BirdContext, result: &TrustedHtml) -> Result<TrustedHtml> {
    let mut context = Context::from_serialize(context)?;
    context.insert("trusted_result", &result.0);
    render_trusted("bird_trusted.html", &context)
}

pub fn render_whois(context: &WhoisContext) -> Result<TrustedHtml> {
    let result = render_formatted_result(&context.result)?;
    let mut context = Context::from_serialize(context)?;
    context.insert("trusted_result", result.as_str());
    render_trusted("whois.html", &context)
}

fn render_formatted_result(result: &str) -> Result<TrustedHtml> {
    let mut context = Context::new();
    context.insert("parts", &formatter::format_result(result)?);
    render_trusted("formatted_result.html", &context)
}

pub fn render_bgpmap(context: &BgpmapContext) -> Result<TrustedHtml> {
    render_trusted("bgpmap.html", &Context::from_serialize(context)?)
}

pub fn render_summary(context: &SummaryContext) -> Result<TrustedHtml> {
    render_trusted("summary.html", &Context::from_serialize(context)?)
}

pub fn render_query_error(context: &QueryErrorContext) -> Result<TrustedHtml> {
    render_trusted("query_error.html", &Context::from_serialize(context)?)
}
