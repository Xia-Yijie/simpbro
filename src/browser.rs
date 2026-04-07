use anyhow::{Context, Result};
use reqwest::blocking::Client;
use url::Url;

use crate::dom::{Dom, NodeId, NodeType};
use crate::js_engine::JsEngine;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Link {
    pub text: String,
    pub url: String,
}

#[allow(dead_code)]
pub struct Page {
    pub url: String,
    pub title: String,
    pub lines: Vec<PageLine>,
    pub links: Vec<Link>,
    pub js_logs: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum PageLine {
    Heading(String, u8),
    Text(String),
    LinkRef(String, usize),
    Blank,
}

const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "meta", "link",
    "head", "textarea", "svg", "iframe",
];

pub struct Browser {
    client: Client,
    pub history: Vec<String>,
}

impl Browser {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("simpbro/0.1")
            .build()?;
        Ok(Self { client, history: Vec::new() })
    }

    pub fn fetch(&mut self, url_str: &str) -> Result<Page> {
        let page = self.fetch_page(url_str)?;
        if self.history.len() >= 100 {
            self.history.remove(0);
        }
        self.history.push(page.url.clone());
        Ok(page)
    }

    pub fn go_back(&mut self) -> Option<Result<Page>> {
        if self.history.len() < 2 {
            return None;
        }
        self.history.pop();
        let prev = self.history.last()?.clone();
        Some(self.fetch_page(&prev))
    }

    fn fetch_page(&self, url_str: &str) -> Result<Page> {
        let url = if url_str.starts_with("http://") || url_str.starts_with("https://") {
            url_str.to_string()
        } else {
            format!("https://{}", url_str)
        };

        let resp = self.client
            .get(&url)
            .send()
            .with_context(|| format!("Failed to fetch {}", url))?;

        let final_url = resp.url().to_string();
        let body = resp.text()?;

        self.parse_page(&body, &final_url)
    }

    fn parse_page(&self, html: &str, base_url: &str) -> Result<Page> {
        let dom = Dom::from_html(html);

        let js_engine = JsEngine::new(dom, base_url, &self.client)?;
        js_engine.execute_scripts()?;

        if let Some(redirect_url) = js_engine.redirected_url() {
            return self.fetch_page(&redirect_url);
        }

        let js_logs = js_engine.logs();
        let dom = js_engine.dom();

        let title = dom.head()
            .and_then(|head| {
                let titles = dom.get_elements_by_tag_name(head, "title");
                titles.first().map(|&id| dom.get_text_content(id).trim().to_string())
            })
            .unwrap_or_default();

        let base = Url::parse(base_url)?;
        let mut lines: Vec<PageLine> = Vec::new();
        let mut links: Vec<Link> = Vec::new();

        if let Some(body_id) = dom.body() {
            Self::walk_dom_node(&dom, body_id, &base, &mut lines, &mut links);
        }

        lines.dedup_by(|a, b| matches!(a, PageLine::Blank) && matches!(b, PageLine::Blank));
        let start = lines.iter().position(|l| !matches!(l, PageLine::Blank)).unwrap_or(lines.len());
        lines.drain(..start);
        while lines.last().is_some_and(|l| matches!(l, PageLine::Blank)) {
            lines.pop();
        }

        Ok(Page { url: base_url.to_string(), title, lines, links, js_logs })
    }

    fn walk_dom_node(
        dom: &Dom,
        node_id: NodeId,
        base: &Url,
        lines: &mut Vec<PageLine>,
        links: &mut Vec<Link>,
    ) {
        let node = &dom.nodes[node_id];

        match node.node_type {
            NodeType::Text => {
                let t = node.text.trim();
                if !t.is_empty() {
                    lines.push(PageLine::Text(t.to_string()));
                }
            }
            NodeType::Element => {
                let tag = node.tag.as_str();

                if SKIP_TAGS.contains(&tag) {
                    return;
                }

                match tag {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = tag[1..].parse::<u8>().unwrap_or(1);
                        let text = dom.get_text_content(node_id).trim().to_string();
                        if !text.is_empty() {
                            lines.push(PageLine::Blank);
                            lines.push(PageLine::Heading(text, level));
                            lines.push(PageLine::Blank);
                        }
                    }
                    "a" => {
                        let text = dom.get_text_content(node_id).trim().to_string();
                        let href = node.attributes.get("href").map(|s| s.as_str()).unwrap_or("");
                        if !text.is_empty() && !href.is_empty() {
                            let resolved = base.join(href).map(|u| u.to_string()).unwrap_or(href.to_string());
                            let idx = links.len();
                            links.push(Link { text: text.clone(), url: resolved });
                            lines.push(PageLine::LinkRef(text, idx));
                        }
                    }
                    "img" => {
                        let alt = node.attributes.get("alt").map(|s| s.as_str()).unwrap_or("图片");
                        lines.push(PageLine::Text(format!("[图: {}]", alt)));
                    }
                    "br" => {
                        lines.push(PageLine::Blank);
                    }
                    "p" | "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                        lines.push(PageLine::Blank);
                        for &child in &node.children {
                            Self::walk_dom_node(dom, child, base, lines, links);
                        }
                        lines.push(PageLine::Blank);
                    }
                    "li" => {
                        let text = dom.get_text_content(node_id).trim().to_string();
                        if !text.is_empty() {
                            lines.push(PageLine::Text(format!("  • {}", text)));
                        }
                    }
                    _ => {
                        for &child in &node.children {
                            Self::walk_dom_node(dom, child, base, lines, links);
                        }
                    }
                }
            }
            NodeType::Document => {
                for &child in &node.children {
                    Self::walk_dom_node(dom, child, base, lines, links);
                }
            }
        }
    }
}
