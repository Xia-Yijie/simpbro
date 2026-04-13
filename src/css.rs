use std::collections::HashMap;

use crate::dom::{Dom, DomNode, NodeId, NodeType};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Display {
    Block,
    Inline,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssColor {
    Named(u8, u8, u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    pub display: Option<Display>,
    pub visibility_hidden: bool,
    pub color: Option<CssColor>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl ComputedStyle {
    pub fn is_hidden(&self) -> bool {
        matches!(self.display, Some(Display::None)) || self.visibility_hidden
    }

    fn inherit_from(&mut self, parent: &ComputedStyle) {
        if self.color.is_none() { self.color = parent.color; }
        if !self.bold { self.bold = parent.bold; }
        if !self.italic { self.italic = parent.italic; }
        if !self.underline { self.underline = parent.underline; }
        if !self.strikethrough { self.strikethrough = parent.strikethrough; }
    }

    fn apply_decl(&mut self, prop: &str, val: &str) {
        match prop {
            "display" => {
                self.display = Some(match val {
                    "none" => Display::None,
                    "inline" | "inline-block" | "inline-flex" => Display::Inline,
                    _ => Display::Block,
                });
            }
            "visibility" => {
                self.visibility_hidden = val == "hidden" || val == "collapse";
            }
            "color" => {
                if let Some(c) = parse_color(val) { self.color = Some(c); }
            }
            "font-weight" => {
                self.bold = matches!(val, "bold" | "bolder" | "700" | "800" | "900");
            }
            "font-style" => {
                self.italic = val == "italic" || val == "oblique";
            }
            "text-decoration" | "text-decoration-line" => {
                self.underline = val.contains("underline");
                self.strikethrough = val.contains("line-through");
            }
            _ => {}
        }
    }
}

// Pre-parsed simple selector
#[derive(Debug, Clone, Default)]
struct SimpleSel {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
}

#[derive(Debug, Clone)]
struct ParsedSelector {
    // Left-to-right parts; last is target, preceding are ancestors (descendant combinator).
    parts: Vec<SimpleSel>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    selector: ParsedSelector,
    declarations: Vec<(String, String)>,
    specificity: u32,
}

pub fn parse_css(text: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut remaining = text;

    loop {
        remaining = skip_comments(remaining.trim_start());
        if remaining.is_empty() { break; }

        if remaining.starts_with('@') {
            remaining = skip_at_rule(remaining);
            continue;
        }

        let brace_open = match remaining.find('{') { Some(b) => b, None => break };
        let mut depth = 1usize;
        let bytes = remaining.as_bytes();
        let mut i = brace_open + 1;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            if depth == 0 { break; }
            i += 1;
        }
        if depth != 0 { break; }

        let selectors_str = remaining[..brace_open].trim();
        let decl_str = &remaining[brace_open + 1..i];
        let declarations = parse_declarations(decl_str);

        if !declarations.is_empty() {
            for selector in selectors_str.split(',') {
                let sel = selector.trim();
                if let Some(parsed) = parse_selector(sel) {
                    let specificity = compute_specificity(sel);
                    rules.push(Rule { selector: parsed, declarations: declarations.clone(), specificity });
                }
                // Unsupported selectors are silently dropped
            }
        }

        remaining = &remaining[(i + 1).min(remaining.len())..];
    }

    rules
}

fn skip_comments(mut s: &str) -> &str {
    while s.starts_with("/*") {
        if let Some(end) = s.find("*/") {
            s = s[end + 2..].trim_start();
        } else {
            return "";
        }
    }
    s
}

fn skip_at_rule(s: &str) -> &str {
    let semi = s.find(';');
    let brace = s.find('{');
    match (semi, brace) {
        (Some(si), None) => &s[si + 1..],
        (Some(si), Some(bi)) if si < bi => &s[si + 1..],
        (_, Some(bi)) => {
            let bytes = s.as_bytes();
            let mut depth = 1usize;
            let mut i = bi + 1;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            &s[i..]
        }
        _ => "",
    }
}

fn parse_declarations(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for decl in text.split(';') {
        if let Some((prop, val)) = decl.split_once(':') {
            let prop = prop.trim().to_lowercase();
            let val = val.trim().trim_end_matches("!important").trim().to_string();
            if !prop.is_empty() && !val.is_empty() {
                out.push((prop, val));
            }
        }
    }
    out
}

/// Parse a selector string. Returns None for unsupported selectors.
fn parse_selector(s: &str) -> Option<ParsedSelector> {
    // Reject features we don't support
    for ch in s.chars() {
        match ch {
            ':' | '[' | '(' | '+' | '~' | '|' | '\\' | '%' | '&' | '@' | '!' => return None,
            _ => {}
        }
    }

    // Split descendant parts (whitespace and >)
    let normalized = s.replace('>', " ");
    let mut parts = Vec::new();
    for token in normalized.split_whitespace() {
        if token == "*" {
            parts.push(SimpleSel::default());
            continue;
        }
        parts.push(parse_simple_selector(token)?);
    }
    if parts.is_empty() { return None; }
    Some(ParsedSelector { parts })
}

fn parse_simple_selector(s: &str) -> Option<SimpleSel> {
    let mut sel = SimpleSel::default();
    let mut start = 0usize;
    let mut kind = 't'; // 't' for tag, '.' for class, '#' for id
    let bytes = s.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        if b == b'.' || b == b'#' {
            if i > start {
                push_part(&mut sel, kind, &s[start..i]);
            }
            start = i + 1;
            kind = b as char;
        }
    }
    if start < s.len() {
        push_part(&mut sel, kind, &s[start..]);
    }

    if sel.tag.is_none() && sel.id.is_none() && sel.classes.is_empty() {
        return None;
    }
    Some(sel)
}

fn push_part(sel: &mut SimpleSel, kind: char, content: &str) {
    if content.is_empty() { return; }
    match kind {
        't' => sel.tag = Some(content.to_lowercase()),
        '#' => sel.id = Some(content.to_string()),
        '.' => sel.classes.push(content.to_string()),
        _ => {}
    }
}

fn compute_specificity(selector: &str) -> u32 {
    let mut spec: u32 = 0;
    for part in selector.split(|c: char| c.is_whitespace() || c == '>' || c == '+' || c == '~') {
        let part = part.trim();
        if part.is_empty() { continue; }
        spec += part.matches('#').count() as u32 * 100;
        spec += part.matches('.').count() as u32 * 10;
        if part.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
            spec += 1;
        }
    }
    spec
}

fn matches_simple(node: &DomNode, sel: &SimpleSel) -> bool {
    if node.node_type != NodeType::Element { return false; }
    if let Some(tag) = &sel.tag {
        if node.tag != *tag { return false; }
    }
    if let Some(id) = &sel.id {
        if node.attributes.get("id").map(|s| s.as_str()) != Some(id.as_str()) { return false; }
    }
    if !sel.classes.is_empty() {
        let class_attr = match node.attributes.get("class") {
            Some(c) => c.as_str(),
            None => return false,
        };
        for required in &sel.classes {
            if !class_attr.split_whitespace().any(|c| c == required) {
                return false;
            }
        }
    }
    true
}

fn matches_parsed(dom: &Dom, node_id: NodeId, sel: &ParsedSelector) -> bool {
    let parts = &sel.parts;
    if parts.is_empty() { return false; }

    // Last part must match target node
    if !matches_simple(&dom.nodes[node_id], parts.last().unwrap()) {
        return false;
    }
    if parts.len() == 1 { return true; }

    // Walk ancestors; preceding parts match right-to-left
    let mut ancestor = dom.nodes[node_id].parent;
    let mut remaining = parts.len() - 1;
    while let Some(aid) = ancestor {
        if matches_simple(&dom.nodes[aid], &parts[remaining - 1]) {
            remaining -= 1;
            if remaining == 0 { return true; }
        }
        ancestor = dom.nodes[aid].parent;
    }
    false
}

/// Parse all `<style>` blocks into sorted rules. Call once per page.
/// The returned rules are pre-sorted ascending by specificity so per-node
/// matching can apply them in order without an extra sort.
pub fn extract_stylesheet_rules(dom: &Dom) -> Vec<Rule> {
    let mut rules = Vec::new();
    for node in &dom.nodes {
        if node.node_type == NodeType::Element && node.tag == "style" {
            let text = dom.get_text_content(node.id);
            rules.extend(parse_css(&text));
        }
    }
    rules.sort_by_key(|r| r.specificity);
    rules
}

/// Compute styles using pre-parsed rules. Inline `style=""` attributes are
/// read fresh since JS can mutate them.
pub fn compute_styles(dom: &Dom, rules: &[Rule]) -> HashMap<NodeId, ComputedStyle> {
    let mut inline: HashMap<NodeId, Vec<(String, String)>> = HashMap::new();
    for node in &dom.nodes {
        if node.node_type != NodeType::Element { continue; }
        if let Some(style_attr) = node.attributes.get("style") {
            let decls = parse_declarations(style_attr);
            if !decls.is_empty() {
                inline.insert(node.id, decls);
            }
        }
    }

    let mut styles: HashMap<NodeId, ComputedStyle> = HashMap::new();

    fn walk(
        dom: &Dom,
        node_id: NodeId,
        parent: Option<&ComputedStyle>,
        rules: &[Rule],
        inline: &HashMap<NodeId, Vec<(String, String)>>,
        styles: &mut HashMap<NodeId, ComputedStyle>,
    ) {
        let node = &dom.nodes[node_id];
        if node.node_type == NodeType::Element {
            let mut style = ComputedStyle::default();

            if let Some(p) = parent {
                style.inherit_from(p);
            }

            apply_tag_defaults(node.tag.as_str(), &mut style);

            // Rules are pre-sorted by specificity, so apply in order.
            for rule in rules.iter().filter(|r| matches_parsed(dom, node_id, &r.selector)) {
                for (prop, val) in &rule.declarations {
                    style.apply_decl(prop, val);
                }
            }

            if let Some(decls) = inline.get(&node_id) {
                for (prop, val) in decls {
                    style.apply_decl(prop, val);
                }
            }

            styles.insert(node_id, style);
        }

        let this_style = styles.get(&node_id).cloned();
        for &child in &dom.nodes[node_id].children {
            walk(dom, child, this_style.as_ref().or(parent), rules, inline, styles);
        }
    }

    walk(dom, 0, None, rules, &inline, &mut styles);
    styles
}

fn apply_tag_defaults(tag: &str, style: &mut ComputedStyle) {
    match tag {
        "b" | "strong" => style.bold = true,
        "i" | "em" => style.italic = true,
        "u" | "ins" => style.underline = true,
        "s" | "del" | "strike" => style.strikethrough = true,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => style.bold = true,
        _ => {}
    }
}

fn parse_color(s: &str) -> Option<CssColor> {
    let s = s.trim().to_lowercase();
    match s.as_str() {
        "black" => return Some(CssColor::Named(0, 0, 0)),
        "white" => return Some(CssColor::Named(255, 255, 255)),
        "red" => return Some(CssColor::Named(255, 0, 0)),
        "green" => return Some(CssColor::Named(0, 128, 0)),
        "blue" => return Some(CssColor::Named(0, 0, 255)),
        "yellow" => return Some(CssColor::Named(255, 255, 0)),
        "cyan" | "aqua" => return Some(CssColor::Named(0, 255, 255)),
        "magenta" | "fuchsia" => return Some(CssColor::Named(255, 0, 255)),
        "gray" | "grey" => return Some(CssColor::Named(128, 128, 128)),
        "silver" => return Some(CssColor::Named(192, 192, 192)),
        "maroon" => return Some(CssColor::Named(128, 0, 0)),
        "olive" => return Some(CssColor::Named(128, 128, 0)),
        "lime" => return Some(CssColor::Named(0, 255, 0)),
        "teal" => return Some(CssColor::Named(0, 128, 128)),
        "navy" => return Some(CssColor::Named(0, 0, 128)),
        "purple" => return Some(CssColor::Named(128, 0, 128)),
        "orange" => return Some(CssColor::Named(255, 165, 0)),
        "transparent" | "inherit" | "currentcolor" => return None,
        _ => {}
    }

    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(CssColor::Rgb(r, g, b));
        }
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            return Some(CssColor::Rgb(r, g, b));
        }
    }

    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r: u8 = parts[0].trim().parse().ok()?;
            let g: u8 = parts[1].trim().parse().ok()?;
            let b: u8 = parts[2].trim().parse().ok()?;
            return Some(CssColor::Rgb(r, g, b));
        }
    }
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|x| x.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r: u8 = parts[0].trim().parse().ok()?;
            let g: u8 = parts[1].trim().parse().ok()?;
            let b: u8 = parts[2].trim().parse().ok()?;
            return Some(CssColor::Rgb(r, g, b));
        }
    }

    None
}
