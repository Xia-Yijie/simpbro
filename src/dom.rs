use std::collections::HashMap;

pub type NodeId = usize;
pub const DOCUMENT_NODE: NodeId = 0;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Document,
    Element,
    Text,
}

#[derive(Debug, Clone)]
pub struct DomNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub tag: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub text: String,
}

pub struct Dom {
    pub nodes: Vec<DomNode>,
}

impl Dom {
    pub fn new() -> Self {
        let doc = DomNode {
            id: 0,
            node_type: NodeType::Document,
            tag: "#document".into(),
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: None,
            text: String::new(),
        };
        Dom { nodes: vec![doc] }
    }

    pub fn create_element(&mut self, tag: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            id,
            node_type: NodeType::Element,
            tag: tag.to_lowercase(),
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: None,
            text: String::new(),
        });
        id
    }

    pub fn create_text_node(&mut self, text: &str) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            id,
            node_type: NodeType::Text,
            tag: "#text".into(),
            attributes: HashMap::new(),
            children: Vec::new(),
            parent: None,
            text: text.to_string(),
        });
        id
    }

    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        if let Some(old_parent) = self.nodes[child_id].parent {
            self.nodes[old_parent].children.retain(|&c| c != child_id);
        }
        self.nodes[child_id].parent = Some(parent_id);
        self.nodes[parent_id].children.push(child_id);
    }

    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        self.nodes[parent_id].children.retain(|&c| c != child_id);
        self.nodes[child_id].parent = None;
    }

    pub fn insert_before(&mut self, parent_id: NodeId, new_child: NodeId, ref_child: NodeId) {
        if let Some(old_parent) = self.nodes[new_child].parent {
            self.nodes[old_parent].children.retain(|&c| c != new_child);
        }
        self.nodes[new_child].parent = Some(parent_id);
        if let Some(pos) = self.nodes[parent_id].children.iter().position(|&c| c == ref_child) {
            self.nodes[parent_id].children.insert(pos, new_child);
        } else {
            self.nodes[parent_id].children.push(new_child);
        }
    }

    pub fn clone_node(&mut self, node_id: NodeId, deep: bool) -> NodeId {
        let node = self.nodes[node_id].clone();
        let new_id = self.nodes.len();
        self.nodes.push(DomNode {
            id: new_id,
            node_type: node.node_type,
            tag: node.tag,
            attributes: node.attributes,
            children: Vec::new(),
            parent: None,
            text: node.text,
        });
        if deep {
            let children: Vec<NodeId> = self.nodes[node_id].children.clone();
            for child_id in children {
                let cloned = self.clone_node(child_id, true);
                self.append_child(new_id, cloned);
            }
        }
        new_id
    }

    // Query methods

    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        self.nodes.iter().find(|n| {
            n.node_type == NodeType::Element
                && n.attributes.get("id").map(|v| v.as_str()) == Some(id)
        }).map(|n| n.id)
    }

    pub fn query_selector(&self, root: NodeId, selector: &str) -> Option<NodeId> {
        let mut results = Vec::new();
        self.find_matching(root, selector, &mut results, true);
        results.into_iter().next()
    }

    pub fn query_selector_all(&self, root: NodeId, selector: &str) -> Vec<NodeId> {
        let mut results = Vec::new();
        self.find_matching(root, selector, &mut results, false);
        results
    }

    fn find_matching(&self, node_id: NodeId, selector: &str, results: &mut Vec<NodeId>, first_only: bool) {
        if first_only && !results.is_empty() {
            return;
        }
        let node = &self.nodes[node_id];
        if node.node_type == NodeType::Element && self.matches_selector(node_id, selector) {
            results.push(node_id);
            if first_only {
                return;
            }
        }
        let children: Vec<NodeId> = node.children.clone();
        for child in children {
            self.find_matching(child, selector, results, first_only);
        }
    }

    pub(crate) fn matches_selector(&self, node_id: NodeId, selector: &str) -> bool {
        let node = &self.nodes[node_id];
        let selector = selector.trim();

        if let Some(id) = selector.strip_prefix('#') {
            return node.attributes.get("id").map(|v| v.as_str()) == Some(id);
        }
        if let Some(class) = selector.strip_prefix('.') {
            return node.attributes.get("class")
                .map(|v| v.split_whitespace().any(|c| c == class))
                .unwrap_or(false);
        }
        // tag.class
        if let Some((tag, class)) = selector.split_once('.') {
            if !tag.is_empty() && !tag.starts_with('#') {
                return node.tag == tag.to_lowercase()
                    && node.attributes.get("class")
                        .map(|v| v.split_whitespace().any(|c| c == class))
                        .unwrap_or(false);
            }
        }
        // tag#id
        if let Some((tag, id)) = selector.split_once('#') {
            if !tag.is_empty() {
                return node.tag == tag.to_lowercase()
                    && node.attributes.get("id").map(|v| v.as_str()) == Some(id);
            }
        }
        // [attr=value]
        if selector.starts_with('[') && selector.ends_with(']') {
            let inner = &selector[1..selector.len()-1];
            if let Some((attr, val)) = inner.split_once('=') {
                let val = val.trim_matches('"').trim_matches('\'');
                return node.attributes.get(attr).map(|v| v.as_str()) == Some(val);
            }
            return node.attributes.contains_key(inner);
        }
        // Simple tag match
        node.tag == selector.to_lowercase()
    }

    pub fn get_elements_by_tag_name(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        let mut results = Vec::new();
        let tag_lower = tag.to_lowercase();
        self.find_by_predicate(root, &|n: &DomNode| {
            n.node_type == NodeType::Element && (tag == "*" || n.tag == tag_lower)
        }, &mut results);
        results
    }

    pub fn get_elements_by_class_name(&self, root: NodeId, class: &str) -> Vec<NodeId> {
        let mut results = Vec::new();
        self.find_by_predicate(root, &|n: &DomNode| {
            n.node_type == NodeType::Element
                && n.attributes.get("class")
                    .map(|v| v.split_whitespace().any(|c| c == class))
                    .unwrap_or(false)
        }, &mut results);
        results
    }

    fn find_by_predicate(&self, node_id: NodeId, pred: &dyn Fn(&DomNode) -> bool, results: &mut Vec<NodeId>) {
        let node = &self.nodes[node_id];
        if pred(node) {
            results.push(node_id);
        }
        let children: Vec<NodeId> = node.children.clone();
        for child in children {
            self.find_by_predicate(child, pred, results);
        }
    }

    // Content accessors

    pub fn get_text_content(&self, node_id: NodeId) -> String {
        let node = &self.nodes[node_id];
        match node.node_type {
            NodeType::Text => node.text.clone(),
            _ => {
                let mut s = String::new();
                for &child in &node.children {
                    s.push_str(&self.get_text_content(child));
                }
                s
            }
        }
    }

    pub fn set_text_content(&mut self, node_id: NodeId, text: &str) {
        let children: Vec<NodeId> = self.nodes[node_id].children.clone();
        for child in children {
            self.nodes[child].parent = None;
        }
        self.nodes[node_id].children.clear();
        if !text.is_empty() {
            let text_id = self.create_text_node(text);
            self.append_child(node_id, text_id);
        }
    }

    pub fn get_inner_html(&self, node_id: NodeId) -> String {
        let mut html = String::new();
        for &child in &self.nodes[node_id].children {
            self.serialize_node(child, &mut html);
        }
        html
    }

    pub fn set_inner_html(&mut self, node_id: NodeId, html: &str) {
        let children: Vec<NodeId> = self.nodes[node_id].children.clone();
        for child in children {
            self.nodes[child].parent = None;
        }
        self.nodes[node_id].children.clear();

        let fragment = scraper::Html::parse_fragment(html);
        self.import_scraper_children(fragment.tree.root(), node_id);
    }

    pub(crate) fn serialize_node(&self, node_id: NodeId, out: &mut String) {
        let node = &self.nodes[node_id];
        match node.node_type {
            NodeType::Text => out.push_str(&node.text),
            NodeType::Element => {
                out.push('<');
                out.push_str(&node.tag);
                for (k, v) in &node.attributes {
                    out.push(' ');
                    out.push_str(k);
                    out.push_str("=\"");
                    out.push_str(v);
                    out.push('"');
                }
                out.push('>');
                for &child in &node.children {
                    self.serialize_node(child, out);
                }
                out.push_str("</");
                out.push_str(&node.tag);
                out.push('>');
            }
            NodeType::Document => {
                for &child in &node.children {
                    self.serialize_node(child, out);
                }
            }
        }
    }

    // Structural accessors

    pub fn body(&self) -> Option<NodeId> {
        for &child in &self.nodes[DOCUMENT_NODE].children {
            if self.nodes[child].tag == "html" {
                for &hc in &self.nodes[child].children {
                    if self.nodes[hc].tag == "body" {
                        return Some(hc);
                    }
                }
            }
        }
        None
    }

    pub fn head(&self) -> Option<NodeId> {
        for &child in &self.nodes[DOCUMENT_NODE].children {
            if self.nodes[child].tag == "html" {
                for &hc in &self.nodes[child].children {
                    if self.nodes[hc].tag == "head" {
                        return Some(hc);
                    }
                }
            }
        }
        None
    }

    // Build from HTML string

    pub fn from_html(html: &str) -> Self {
        let document = scraper::Html::parse_document(html);
        let mut dom = Dom::new();
        dom.import_scraper_children(document.tree.root(), DOCUMENT_NODE);
        dom
    }

    fn import_scraper_children(&mut self, scraper_node: ego_tree::NodeRef<scraper::Node>, parent_id: NodeId) {
        for child in scraper_node.children() {
            match child.value() {
                scraper::Node::Document => {
                    self.import_scraper_children(child, parent_id);
                }
                scraper::Node::Element(el) => {
                    let node_id = self.create_element(el.name());
                    for (name, value) in el.attrs() {
                        self.nodes[node_id].attributes.insert(name.to_string(), value.to_string());
                    }
                    self.append_child(parent_id, node_id);
                    self.import_scraper_children(child, node_id);
                }
                scraper::Node::Text(text) => {
                    let node_id = self.create_text_node(text);
                    self.append_child(parent_id, node_id);
                }
                _ => {}
            }
        }
    }


    /// Extract all <script> tags, returning (src_or_none, inline_code_or_none) pairs
    pub fn extract_scripts(&self) -> Vec<(Option<String>, Option<String>)> {
        let mut scripts = Vec::new();
        self.collect_scripts(DOCUMENT_NODE, &mut scripts);
        scripts
    }

    fn collect_scripts(&self, node_id: NodeId, scripts: &mut Vec<(Option<String>, Option<String>)>) {
        let node = &self.nodes[node_id];
        if node.node_type == NodeType::Element && node.tag == "script" {
            let src = node.attributes.get("src").cloned();
            let inline = if src.is_none() {
                let text = self.get_text_content(node_id);
                let trimmed = text.trim();
                if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
            } else {
                None
            };
            scripts.push((src, inline));
        }
        let children: Vec<NodeId> = node.children.clone();
        for child in children {
            self.collect_scripts(child, scripts);
        }
    }
}
