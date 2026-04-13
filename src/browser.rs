use anyhow::{Context, Result};
use reqwest::blocking::Client;
use url::Url;

use std::collections::{HashMap, HashSet};

use crate::css::{self, ComputedStyle, CssColor, Rule};
use crate::dom::{Dom, DomNode, NodeId, NodeType};
use crate::js_engine::JsEngine;

#[derive(Clone, Debug, Default)]
pub struct TextStyle {
    pub color: Option<(u8, u8, u8)>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl TextStyle {
    fn from_computed(cs: &ComputedStyle) -> Self {
        Self {
            color: cs.color.map(|c| match c {
                CssColor::Named(r, g, b) | CssColor::Rgb(r, g, b) => (r, g, b),
            }),
            bold: cs.bold,
            italic: cs.italic,
            underline: cs.underline,
            strikethrough: cs.strikethrough,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Link {
    pub node_id: NodeId,
    pub text: String,
    pub url: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FormInput {
    pub node_id: NodeId,
    pub input_type: String,
    pub name: String,
    pub value: String,
    pub placeholder: String,
    pub form_id: Option<usize>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FormInfo {
    pub node_id: NodeId,
    pub action: String,
    pub method: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Button {
    pub node_id: NodeId,
    pub label: String,
    pub kind: ButtonKind,
    pub form_id: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Submit,
    Plain,
}

#[derive(Clone, Debug)]
pub enum PageLine {
    Heading(String, u8, TextStyle),
    Text(String, TextStyle),
    LinkRef(String, usize, TextStyle),
    InputRef(String, usize, TextStyle),
    ButtonRef(String, usize, TextStyle),
    Blank,
}

const SKIP_TAGS: &[&str] = &[
    "script", "style", "noscript", "meta", "link",
    "head", "textarea", "svg", "iframe",
];

/// A fetched, parsed, interactive page. Holds a live JS engine and DOM.
pub struct Page {
    pub url: String,
    pub title: String,
    pub lines: Vec<PageLine>,
    pub links: Vec<Link>,
    pub inputs: Vec<FormInput>,
    pub forms: Vec<FormInfo>,
    pub buttons: Vec<Button>,
    #[allow(dead_code)]
    pub js_logs: Vec<String>,
    engine: JsEngine,
    // Stylesheet rules parsed once at page build time.
    stylesheet: Vec<Rule>,
}

impl Page {
    /// Re-walk the DOM to refresh lines/links/inputs/forms (call after JS mutations).
    pub fn render(&mut self) {
        let base = match Url::parse(&self.url) {
            Ok(u) => u,
            Err(_) => return,
        };
        let click_nodes = self.engine.nodes_with_listener("click");
        let dom = self.engine.dom();
        let styles = css::compute_styles(&dom, &self.stylesheet);
        let has_interactive = compute_interactive_ancestors(&dom);

        self.title = dom.head()
            .and_then(|head| {
                let titles = dom.get_elements_by_tag_name(head, "title");
                titles.first().map(|&id| dom.get_text_content(id).trim().to_string())
            })
            .unwrap_or_default();

        let mut ctx = WalkCtx::default();
        if let Some(body_id) = dom.body() {
            let input = WalkInput {
                dom: &dom,
                base: &base,
                styles: &styles,
                click_nodes: &click_nodes,
                has_interactive: &has_interactive,
            };
            walk_dom_node(&input, body_id, &mut ctx, None);
        }

        ctx.lines.dedup_by(|a, b| matches!(a, PageLine::Blank) && matches!(b, PageLine::Blank));
        let start = ctx.lines.iter().position(|l| !matches!(l, PageLine::Blank)).unwrap_or(ctx.lines.len());
        ctx.lines.drain(..start);
        while ctx.lines.last().is_some_and(|l| matches!(l, PageLine::Blank)) {
            ctx.lines.pop();
        }

        self.lines = ctx.lines;
        self.links = ctx.links;
        self.inputs = ctx.inputs;
        self.forms = ctx.forms;
        self.buttons = ctx.buttons;
        self.js_logs = self.engine.logs();
    }

    /// Dispatch a click on a link. Returns Some(url) if default proceeds (navigate),
    /// None if JS called preventDefault.
    pub fn click_link(&mut self, link_idx: usize) -> Option<String> {
        let node_id = self.links.get(link_idx)?.node_id;
        let proceed = self.engine.dispatch_event(node_id, "click");
        self.render();
        if !proceed { return None; }
        // After render, links[link_idx].url reflects any JS-modified href.
        self.links.get(link_idx).map(|l| l.url.clone())
    }

    /// Set an input's value and dispatch input/change events.
    pub fn set_input_value(&mut self, input_idx: usize, value: String) {
        let node_id = match self.inputs.get(input_idx) {
            Some(inp) => inp.node_id,
            None => return,
        };
        self.engine.set_attribute(node_id, "value", &value);
        self.engine.dispatch_event(node_id, "input");
        self.engine.dispatch_event(node_id, "change");
        self.render();
    }

    /// Click a button. For submit buttons, dispatches click then submits the form.
    /// Returns Some(url) if form should navigate.
    pub fn click_button(&mut self, button_idx: usize) -> Option<String> {
        let button = self.buttons.get(button_idx)?.clone();
        let proceed = self.engine.dispatch_event(button.node_id, "click");
        if !proceed || button.kind != ButtonKind::Submit {
            self.render();
            return None;
        }
        // For submits, skip the intermediate render — submit_form_by_idx will render.
        let form_idx = button.form_id?;
        self.submit_form_by_idx(form_idx)
    }

    fn submit_form_by_idx(&mut self, form_idx: usize) -> Option<String> {
        let form_node_id = self.forms.get(form_idx)?.node_id;
        let proceed = self.engine.dispatch_event(form_node_id, "submit");
        self.render();
        if proceed {
            let form = self.forms.get(form_idx)?;
            Browser::build_form_url(form, &self.inputs, form_idx)
        } else {
            None
        }
    }

    /// Submit the form containing the given input. Returns Some(url) if default proceeds.
    pub fn submit_form(&mut self, input_idx: usize) -> Option<String> {
        let form_idx = self.inputs.get(input_idx)?.form_id?;
        self.submit_form_by_idx(form_idx)
    }

}

#[derive(Default)]
struct WalkCtx {
    lines: Vec<PageLine>,
    links: Vec<Link>,
    inputs: Vec<FormInput>,
    forms: Vec<FormInfo>,
    buttons: Vec<Button>,
}

fn is_js_clickable(node: &DomNode, click_nodes: &HashSet<NodeId>) -> bool {
    if node.attributes.contains_key("onclick") { return true; }
    if click_nodes.contains(&node.id) { return true; }
    if let Some(role) = node.attributes.get("role") {
        if role == "button" || role == "link" { return true; }
    }
    false
}

/// Single bottom-up pass: returns the set of nodes that have at least one
/// interactive (a/input/button/form/select/textarea) descendant.
fn compute_interactive_ancestors(dom: &Dom) -> HashSet<NodeId> {
    let mut set = HashSet::new();
    fn visit(dom: &Dom, node_id: NodeId, set: &mut HashSet<NodeId>) -> bool {
        let node = &dom.nodes[node_id];
        let mut has = false;
        for &child_id in &node.children {
            let child = &dom.nodes[child_id];
            let is_interactive = child.node_type == NodeType::Element
                && matches!(child.tag.as_str(), "a" | "input" | "button" | "form" | "select" | "textarea");
            let child_has = visit(dom, child_id, set);
            if is_interactive || child_has {
                has = true;
            }
        }
        if has { set.insert(node_id); }
        has
    }
    visit(dom, 0, &mut set);
    set
}

struct WalkInput<'a> {
    dom: &'a Dom,
    base: &'a Url,
    styles: &'a HashMap<NodeId, ComputedStyle>,
    click_nodes: &'a HashSet<NodeId>,
    has_interactive: &'a HashSet<NodeId>,
}

fn walk_dom_node(
    w: &WalkInput,
    node_id: NodeId,
    ctx: &mut WalkCtx,
    current_form: Option<usize>,
) {
    let dom = w.dom;
    let base = w.base;
    let styles = w.styles;
    let click_nodes = w.click_nodes;
    let has_interactive = w.has_interactive;
    let node = &dom.nodes[node_id];

    match node.node_type {
        NodeType::Text => {
            let t = node.text.trim();
            if !t.is_empty() {
                // Inherit style from parent element
                let parent_style = node.parent.and_then(|p| styles.get(&p))
                    .map(TextStyle::from_computed)
                    .unwrap_or_default();
                ctx.lines.push(PageLine::Text(t.to_string(), parent_style));
            }
        }
        NodeType::Element => {
            let tag = node.tag.as_str();

            // Skip hidden elements, except forms — real pages often ship forms
            // with display:none that JS un-hides at runtime; skipping them would
            // drop the real input/button inside.
            if let Some(cs) = styles.get(&node_id) {
                if cs.is_hidden() && node.tag != "form" {
                    return;
                }
            }

            let ts = styles.get(&node_id).map(TextStyle::from_computed).unwrap_or_default();

            if tag == "textarea" {
                let name = node.attributes.get("name").cloned().unwrap_or_default();
                let placeholder = node.attributes.get("placeholder").cloned().unwrap_or_default();
                let value = dom.get_text_content(node_id).trim().to_string();
                if (value.contains('{') && value.contains('}')) && (value.contains('<') || value.contains(';')) {
                    return;
                }
                let idx = ctx.inputs.len();
                let display = if placeholder.is_empty() { format!("文本框:{}", name) } else { placeholder.clone() };
                ctx.inputs.push(FormInput { node_id, input_type: "textarea".into(), name, value, placeholder, form_id: current_form });
                ctx.lines.push(PageLine::InputRef(display, idx, ts));
                return;
            }

            if SKIP_TAGS.contains(&tag) {
                return;
            }

            // Generic clickable elements (div/span/li/... with onclick, JS listener,
            // or role=button/link). Emit as ButtonRef and skip children — but only
            // if the subtree has no nested interactive elements we'd otherwise render.
            let generic_clickable = is_js_clickable(node, click_nodes)
                && !matches!(tag, "a" | "input" | "button" | "form" | "select" | "textarea")
                && !has_interactive.contains(&node_id);
            if generic_clickable {
                let text = dom.get_text_content(node_id).trim().to_string();
                if !text.is_empty() {
                    let label = if text.chars().count() > 50 {
                        text.chars().take(50).collect::<String>() + "…"
                    } else {
                        text
                    };
                    let idx = ctx.buttons.len();
                    ctx.buttons.push(Button {
                        node_id,
                        label: label.clone(),
                        kind: ButtonKind::Plain,
                        form_id: current_form,
                    });
                    ctx.lines.push(PageLine::ButtonRef(label, idx, ts));
                    return;
                }
            }

            match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag[1..].parse::<u8>().unwrap_or(1);
                    let text = dom.get_text_content(node_id).trim().to_string();
                    if !text.is_empty() {
                        ctx.lines.push(PageLine::Blank);
                        ctx.lines.push(PageLine::Heading(text, level, ts));
                        ctx.lines.push(PageLine::Blank);
                    }
                }
                "a" => {
                    let text = dom.get_text_content(node_id).trim().to_string();
                    let href = node.attributes.get("href").map(|s| s.as_str()).unwrap_or("");
                    if !text.is_empty() && !href.is_empty() {
                        let resolved = base.join(href).map(|u| u.to_string()).unwrap_or(href.to_string());
                        let idx = ctx.links.len();
                        ctx.links.push(Link { node_id, text: text.clone(), url: resolved });
                        ctx.lines.push(PageLine::LinkRef(text, idx, ts));
                    }
                }
                "form" => {
                    let action = node.attributes.get("action")
                        .and_then(|a| base.join(a).ok())
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| base.to_string());
                    let method = node.attributes.get("method")
                        .map(|m| m.to_uppercase())
                        .unwrap_or_else(|| "GET".into());
                    let form_idx = ctx.forms.len();
                    ctx.forms.push(FormInfo { node_id, action, method });
                    for &child in &node.children {
                        walk_dom_node(w, child, ctx, Some(form_idx));
                    }
                }
                "input" => {
                    let input_type = node.attributes.get("type").map(|s| s.as_str()).unwrap_or("text");
                    if matches!(input_type, "hidden" | "image" | "reset" | "checkbox" | "radio" | "file") {
                        return;
                    }
                    if input_type == "submit" || input_type == "button" {
                        let label = node.attributes.get("value").map(|s| s.as_str()).unwrap_or("提交").to_string();
                        let idx = ctx.buttons.len();
                        let kind = if input_type == "submit" { ButtonKind::Submit } else { ButtonKind::Plain };
                        ctx.buttons.push(Button { node_id, label: label.clone(), kind, form_id: current_form });
                        ctx.lines.push(PageLine::ButtonRef(label, idx, ts));
                        return;
                    }
                    let name = node.attributes.get("name").cloned().unwrap_or_default();
                    let value = node.attributes.get("value").cloned().unwrap_or_default();
                    let placeholder = node.attributes.get("placeholder").cloned().unwrap_or_default();
                    let idx = ctx.inputs.len();
                    let display = if placeholder.is_empty() { format!("输入框:{}", name) } else { placeholder.clone() };
                    ctx.inputs.push(FormInput { node_id, input_type: input_type.to_string(), name, value, placeholder, form_id: current_form });
                    ctx.lines.push(PageLine::InputRef(display, idx, ts));
                }
                "button" => {
                    let label = dom.get_text_content(node_id).trim().to_string();
                    let label = if label.is_empty() { "按钮".to_string() } else { label };
                    let btype = node.attributes.get("type").map(|s| s.as_str()).unwrap_or("submit");
                    let kind = if btype == "submit" { ButtonKind::Submit } else { ButtonKind::Plain };
                    let idx = ctx.buttons.len();
                    ctx.buttons.push(Button { node_id, label: label.clone(), kind, form_id: current_form });
                    ctx.lines.push(PageLine::ButtonRef(label, idx, ts));
                }
                "img" => {
                    let alt = node.attributes.get("alt").map(|s| s.as_str()).unwrap_or("图片");
                    ctx.lines.push(PageLine::Text(format!("[图: {}]", alt), ts));
                }
                "br" => {
                    ctx.lines.push(PageLine::Blank);
                }
                "p" | "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                    ctx.lines.push(PageLine::Blank);
                    for &child in &node.children {
                        walk_dom_node(w, child, ctx, current_form);
                    }
                    ctx.lines.push(PageLine::Blank);
                }
                "li" => {
                    let text = dom.get_text_content(node_id).trim().to_string();
                    if !text.is_empty() {
                        ctx.lines.push(PageLine::Text(format!("  • {}", text), ts));
                    }
                }
                _ => {
                    for &child in &node.children {
                        walk_dom_node(w, child, ctx, current_form);
                    }
                }
            }
        }
        NodeType::Document => {
            for &child in &node.children {
                walk_dom_node(w, child, ctx, current_form);
            }
        }
    }
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

        self.build_page(&body, &final_url)
    }

    fn build_page(&self, html: &str, base_url: &str) -> Result<Page> {
        let dom = Dom::from_html(html);
        let engine = JsEngine::new(dom, base_url, &self.client)?;
        engine.execute_scripts()?;

        if let Some(redirect_url) = engine.redirected_url() {
            return self.fetch_page(&redirect_url);
        }

        let stylesheet = css::extract_stylesheet_rules(&engine.dom());

        let mut page = Page {
            url: base_url.to_string(),
            title: String::new(),
            lines: Vec::new(),
            links: Vec::new(),
            inputs: Vec::new(),
            forms: Vec::new(),
            buttons: Vec::new(),
            js_logs: Vec::new(),
            engine,
            stylesheet,
        };
        page.render();
        Ok(page)
    }

    pub fn build_form_url(form: &FormInfo, inputs: &[FormInput], form_idx: usize) -> Option<String> {
        let mut url = Url::parse(&form.action).ok()?;
        {
            let mut query = url.query_pairs_mut();
            for inp in inputs.iter().filter(|i| i.form_id == Some(form_idx) && !i.name.is_empty()) {
                query.append_pair(&inp.name, &inp.value);
            }
        }
        Some(url.to_string())
    }
}
