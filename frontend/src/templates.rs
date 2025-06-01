use anyhow::Result;
use serde::Serialize;
use std::sync::OnceLock;
use tera::{Context, Tera};

static TEMPLATES: OnceLock<Tera> = OnceLock::new();

#[derive(Serialize)]
pub struct PageContext {
    pub title: String,
    pub brand: String,
    pub brand_url: String,
    pub all_server_title: String,
    pub all_servers_url: String,
    pub all_servers_link_active: bool,
    pub servers: Vec<String>,
    pub servers_display: Vec<String>,
    pub url_option: String,
    pub url_server: String,
    pub url_command: String,
    pub options: Vec<(String, String)>,
    pub content: String,
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
pub struct SummaryContext {
    pub server_name: String,
    pub headers: Vec<String>,
    pub rows: Vec<SummaryRowData>,
}

#[derive(Serialize)]
pub struct SummaryRowData {
    pub name: String,
    pub proto: String,
    pub table: String,
    pub state: String,
    pub mapped_state: String,
    pub since: String,
    pub info: String,
}

pub fn init() -> Result<()> {
    let mut tera = Tera::new("frontend/assets/templates/**/*")?;
    tera.autoescape_on(vec!["html"]);
    
    TEMPLATES.set(tera).map_err(|_| anyhow::anyhow!("Templates already initialized"))?;
    Ok(())
}

pub fn get_templates() -> &'static Tera {
    TEMPLATES.get().expect("Templates not initialized")
}

pub fn render_page(context: &PageContext) -> Result<String> {
    let tera = get_templates();
    let rendered = tera.render("page.html", &Context::from_serialize(context)?)?;
    Ok(rendered)
}

pub fn render_bird(context: &BirdContext) -> Result<String> {
    let tera = get_templates();
    let rendered = tera.render("bird.html", &Context::from_serialize(context)?)?;
    Ok(rendered)
}

pub fn render_whois(context: &WhoisContext) -> Result<String> {
    let tera = get_templates();
    let rendered = tera.render("whois.html", &Context::from_serialize(context)?)?;
    Ok(rendered)
}

pub fn render_bgpmap(context: &BgpmapContext) -> Result<String> {
    let tera = get_templates();
    let rendered = tera.render("bgpmap.html", &Context::from_serialize(context)?)?;
    Ok(rendered)
}

pub fn render_summary(context: &SummaryContext) -> Result<String> {
    let tera = get_templates();
    let rendered = tera.render("summary.html", &Context::from_serialize(context)?)?;
    Ok(rendered)
} 