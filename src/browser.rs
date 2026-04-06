use anyhow::{Context, Result};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use url::Url;

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

        self.history.push(final_url.clone());

        let page = self.parse_html(&body, &final_url)?;
        Ok(page)
    }

    fn parse_html(&self, html: &str, base_url: &str) -> Result<Page> {
        let document = Html::parse_document(html);
        let base = Url::parse(base_url)?;

        // Extract title
        let title_sel = Selector::parse("title").unwrap();
        let title = document
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let mut lines: Vec<PageLine> = Vec::new();
        let mut links: Vec<Link> = Vec::new();

        // Parse body content
        let body_sel = Selector::parse("body").unwrap();
        if let Some(body) = document.select(&body_sel).next() {
            self.walk_node(&body, &base, &mut lines, &mut links);
        }

        // Remove leading/trailing blanks
        while lines.first().is_some_and(|l| matches!(l, PageLine::Blank)) {
            lines.remove(0);
        }
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

    fn walk_node(
        &self,
        element: &scraper::ElementRef,
        base: &Url,
        lines: &mut Vec<PageLine>,
        links: &mut Vec<Link>,
    ) {
        use scraper::Node;

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

                    // Skip non-visible elements
                    if matches!(tag, "script" | "style" | "noscript" | "meta" | "link" | "head") {
                        continue;
                    }

                    let child_ref = scraper::ElementRef::wrap(child);
                    if let Some(child_el) = child_ref {
                        match tag {
                            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                                let level = tag[1..].parse::<u8>().unwrap_or(1);
                                let text: String = child_el.text().collect::<String>().trim().to_string();
                                if !text.is_empty() {
                                    lines.push(PageLine::Blank);
                                    lines.push(PageLine::Heading(text, level));
                                    lines.push(PageLine::Blank);
                                }
                            }
                            "a" => {
                                let text: String = child_el.text().collect::<String>().trim().to_string();
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
                                let text: String = child_el.text().collect::<String>().trim().to_string();
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
