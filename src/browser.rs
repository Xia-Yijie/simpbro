use std::sync::OnceLock;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use url::Url;

fn title_selector() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("title").unwrap())
}

fn body_selector() -> &'static Selector {
    static SEL: OnceLock<Selector> = OnceLock::new();
    SEL.get_or_init(|| Selector::parse("body").unwrap())
}

/// A parsed link extracted from HTML
#[derive(Clone, Debug)]
pub struct Link {
    pub text: String,
    pub url: String,
}

/// Represents a fetched and parsed page
pub struct Page {
    pub url: String,
    pub title: String,
    pub lines: Vec<PageLine>,
    pub links: Vec<Link>,
}

/// A single line of rendered content
#[derive(Clone, Debug)]
pub enum PageLine {
    Heading(String, u8),   // text, level (1-6)
    Text(String),
    LinkRef(String, usize), // text, link index
    Blank,
}

pub struct Browser {
    client: Client,
    pub history: Vec<String>,
}

impl Browser {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent("simpbro/0.1")
            .build()?;
        Ok(Self {
            client,
            history: Vec::new(),
        })
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

        let resp = self
            .client
            .get(&url)
            .send()
            .with_context(|| format!("Failed to fetch {}", url))?;

        let final_url = resp.url().to_string();
        let body = resp.text()?;

        self.parse_html(&body, &final_url)
    }

    fn parse_html(&self, html: &str, base_url: &str) -> Result<Page> {
        let document = Html::parse_document(html);
        let base = Url::parse(base_url)?;

        let title = document
            .select(title_selector())
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let mut lines: Vec<PageLine> = Vec::new();
        let mut links: Vec<Link> = Vec::new();

        if let Some(body) = document.select(body_selector()).next() {
            self.walk_node(&body, &base, &mut lines, &mut links);
        }

        lines.dedup_by(|a, b| matches!(a, PageLine::Blank) && matches!(b, PageLine::Blank));

        let start = lines.iter().position(|l| !matches!(l, PageLine::Blank)).unwrap_or(lines.len());
        lines.drain(..start);
        while lines.last().is_some_and(|l| matches!(l, PageLine::Blank)) {
            lines.pop();
        }

        Ok(Page {
            url: base_url.to_string(),
            title,
            lines,
            links,
        })
    }

    fn collect_text(el: &scraper::ElementRef) -> String {
        el.text().collect::<String>().trim().to_string()
    }

    fn walk_node(
        &self,
        element: &scraper::ElementRef,
        base: &Url,
        lines: &mut Vec<PageLine>,
        links: &mut Vec<Link>,
    ) {
        use scraper::Node;

        const SKIP_TAGS: &[&str] = &[
            "script", "style", "noscript", "meta", "link",
            "head", "textarea", "svg", "iframe",
        ];

        for child in element.children() {
            match child.value() {
                Node::Text(text) => {
                    let t = text.trim();
                    if !t.is_empty() {
                        lines.push(PageLine::Text(t.to_string()));
                    }
                }
                Node::Element(el) => {
                    let tag = el.name();

                    if SKIP_TAGS.contains(&tag) {
                        continue;
                    }

                    let child_ref = scraper::ElementRef::wrap(child);
                    if let Some(child_el) = child_ref {
                        match tag {
                            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                                let level = tag[1..].parse::<u8>().unwrap_or(1);
                                let text: String = Self::collect_text(&child_el);
                                if !text.is_empty() {
                                    lines.push(PageLine::Blank);
                                    lines.push(PageLine::Heading(text, level));
                                    lines.push(PageLine::Blank);
                                }
                            }
                            "a" => {
                                let text: String = Self::collect_text(&child_el);
                                let href = el.attr("href").unwrap_or("");
                                if !text.is_empty() && !href.is_empty() {
                                    let resolved = base.join(href).map(|u| u.to_string()).unwrap_or(href.to_string());
                                    let idx = links.len();
                                    links.push(Link {
                                        text: text.clone(),
                                        url: resolved,
                                    });
                                    lines.push(PageLine::LinkRef(text, idx));
                                }
                            }
                            "br" => {
                                lines.push(PageLine::Blank);
                            }
                            "p" | "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                                lines.push(PageLine::Blank);
                                self.walk_node(&child_el, base, lines, links);
                                lines.push(PageLine::Blank);
                            }
                            "li" => {
                                let text: String = Self::collect_text(&child_el);
                                if !text.is_empty() {
                                    lines.push(PageLine::Text(format!("  • {}", text)));
                                }
                            }
                            _ => {
                                self.walk_node(&child_el, base, lines, links);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
