use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use reqwest::blocking::Client;
use rquickjs::{Context, Function, Runtime, Value};

use crate::dom::{Dom, NodeType, DOCUMENT_NODE};

struct TimerEntry {
    id: i32,
    ms: i32,
    #[allow(dead_code)]
    interval: bool,
}

pub struct JsEngine {
    #[allow(dead_code)]
    runtime: Runtime,
    context: Context,
    dom: Rc<RefCell<Dom>>,
    timers: Rc<RefCell<Vec<TimerEntry>>>,
    logs: Rc<RefCell<Vec<String>>>,
    client: Client,
}

impl JsEngine {
    pub fn new(dom: Dom, page_url: &str, client: &Client) -> Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;

        let dom = Rc::new(RefCell::new(dom));
        let timers: Rc<RefCell<Vec<TimerEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let logs: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        let url = page_url.to_string();
        let http_client = client.clone();

        let dom_c = dom.clone();
        let timers_c = timers.clone();
        let logs_c = logs.clone();

        context.with(|ctx| -> Result<()> {
            let globals = ctx.globals();
            globals.set("__page_url", url)?;

            // === DOM functions ===
            macro_rules! dom_fn {
                ($name:expr, $closure:expr) => {
                    globals.set($name, Function::new(ctx.clone(), $closure))?;
                };
            }

            let d = dom_c.clone();
            dom_fn!("__dom_document_id", move || -> i32 { let _ = &d; DOCUMENT_NODE as i32 });

            let d = dom_c.clone();
            dom_fn!("__dom_create_element", move |tag: String| -> i32 {
                d.borrow_mut().create_element(&tag) as i32
            });

            let d = dom_c.clone();
            dom_fn!("__dom_create_text_node", move |text: String| -> i32 {
                d.borrow_mut().create_text_node(&text) as i32
            });

            let d = dom_c.clone();
            dom_fn!("__dom_append_child", move |parent: i32, child: i32| {
                d.borrow_mut().append_child(parent as usize, child as usize);
            });

            let d = dom_c.clone();
            dom_fn!("__dom_remove_child", move |parent: i32, child: i32| {
                d.borrow_mut().remove_child(parent as usize, child as usize);
            });

            let d = dom_c.clone();
            dom_fn!("__dom_insert_before", move |parent: i32, new_child: i32, ref_child: i32| {
                let mut dom = d.borrow_mut();
                if ref_child < 0 {
                    dom.append_child(parent as usize, new_child as usize);
                } else {
                    dom.insert_before(parent as usize, new_child as usize, ref_child as usize);
                }
            });

            let d = dom_c.clone();
            dom_fn!("__dom_clone_node", move |id: i32, deep: bool| -> i32 {
                d.borrow_mut().clone_node(id as usize, deep) as i32
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_attr", move |id: i32, name: String| -> Option<String> {
                d.borrow().nodes.get(id as usize)
                    .and_then(|n| n.attributes.get(&name).cloned())
            });

            let d = dom_c.clone();
            dom_fn!("__dom_set_attr", move |id: i32, name: String, value: String| {
                d.borrow_mut().nodes[id as usize].attributes.insert(name, value);
            });

            let d = dom_c.clone();
            dom_fn!("__dom_remove_attr", move |id: i32, name: String| {
                d.borrow_mut().nodes[id as usize].attributes.remove(&name);
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_text_content", move |id: i32| -> String {
                d.borrow().get_text_content(id as usize)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_set_text_content", move |id: i32, text: String| {
                d.borrow_mut().set_text_content(id as usize, &text);
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_inner_html", move |id: i32| -> String {
                d.borrow().get_inner_html(id as usize)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_set_inner_html", move |id: i32, html: String| {
                d.borrow_mut().set_inner_html(id as usize, &html);
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_outer_html", move |id: i32| -> String {
                let dom = d.borrow();
                let mut html = String::new();
                dom.serialize_node(id as usize, &mut html);
                html
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_children", move |id: i32| -> Vec<i32> {
                let dom = d.borrow();
                dom.nodes[id as usize].children.iter()
                    .filter(|&&c| dom.nodes[c].node_type == NodeType::Element)
                    .map(|&c| c as i32)
                    .collect()
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_child_nodes", move |id: i32| -> Vec<i32> {
                d.borrow().nodes[id as usize].children.iter().map(|&c| c as i32).collect()
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_parent", move |id: i32| -> i32 {
                d.borrow().nodes[id as usize].parent.map(|p| p as i32).unwrap_or(-1)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_tag", move |id: i32| -> String {
                d.borrow().nodes[id as usize].tag.clone()
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_node_type", move |id: i32| -> i32 {
                match d.borrow().nodes[id as usize].node_type {
                    NodeType::Element => 1,
                    NodeType::Text => 3,
                    NodeType::Document => 9,
                }
            });

            let d = dom_c.clone();
            dom_fn!("__dom_query_selector", move |root: i32, sel: String| -> i32 {
                d.borrow().query_selector(root as usize, &sel).map(|id| id as i32).unwrap_or(-1)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_query_selector_all", move |root: i32, sel: String| -> Vec<i32> {
                d.borrow().query_selector_all(root as usize, &sel).into_iter().map(|id| id as i32).collect()
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_element_by_id", move |id: String| -> i32 {
                d.borrow().get_element_by_id(&id).map(|n| n as i32).unwrap_or(-1)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_by_tag", move |root: i32, tag: String| -> Vec<i32> {
                d.borrow().get_elements_by_tag_name(root as usize, &tag).into_iter().map(|id| id as i32).collect()
            });

            let d = dom_c.clone();
            dom_fn!("__dom_get_by_class", move |root: i32, cls: String| -> Vec<i32> {
                d.borrow().get_elements_by_class_name(root as usize, &cls).into_iter().map(|id| id as i32).collect()
            });

            let d = dom_c.clone();
            dom_fn!("__dom_body", move || -> i32 {
                d.borrow().body().map(|id| id as i32).unwrap_or(-1)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_head", move || -> i32 {
                d.borrow().head().map(|id| id as i32).unwrap_or(-1)
            });

            let d = dom_c.clone();
            dom_fn!("__dom_matches", move |id: i32, sel: String| -> bool {
                d.borrow().matches_selector(id as usize, &sel)
            });

            // === Console ===
            let l = logs_c.clone();
            globals.set("__console_log", Function::new(ctx.clone(), move |msg: String| {
                l.borrow_mut().push(msg);
            }))?;

            // === Timers ===
            let t = timers_c.clone();
            globals.set("__register_timer", Function::new(ctx.clone(), move |id: i32, ms: i32, interval: bool| {
                t.borrow_mut().push(TimerEntry { id, ms, interval });
            }))?;

            let fetch_client = http_client.clone();
            globals.set("__fetch_sync_raw", Function::new(ctx.clone(), move |url: String, method: String, headers_json: String, body: String| -> String {
                let client = &fetch_client;

                let req = match method.to_uppercase().as_str() {
                    "POST" => client.post(&url).body(body),
                    "PUT" => client.put(&url).body(body),
                    "DELETE" => client.delete(&url),
                    _ => client.get(&url),
                };

                let req = if let Ok(headers) = serde_json::from_str::<std::collections::HashMap<String, String>>(&headers_json) {
                    headers.into_iter().fold(req, |r, (k, v)| r.header(&k, &v))
                } else {
                    req
                };

                match req.send() {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let final_url = resp.url().to_string();
                        let body = resp.text().unwrap_or_default();
                        let body_escaped = serde_json::to_string(&body).unwrap_or_else(|_| "\"\"".to_string());
                        format!(r#"{{"status":{},"body":{},"url":"{}","headers":{{}}}}"#, status, body_escaped, final_url)
                    }
                    Err(e) => {
                        let err_msg = serde_json::to_string(&format!("Fetch error: {}", e)).unwrap_or_else(|_| "\"\"".to_string());
                        format!(r#"{{"status":0,"body":{},"url":"{}","headers":{{}}}}"#, err_msg, url)
                    }
                }
            }))?;

            ctx.eval::<(), _>(r#"
                globalThis.__fetch_sync = function(url, method, headers, body) {
                    const raw = __fetch_sync_raw(url, method, headers, body);
                    return JSON.parse(raw);
                };
            "#)?;

            // === Run polyfill ===
            let polyfill = include_str!("js_polyfill.js");
            ctx.eval::<(), _>(polyfill)?;

            Ok(())
        })?;

        Ok(Self { runtime, context, dom, timers, logs, client: http_client })
    }

    fn resolve_script_url(page_url: &str, src_url: &str) -> Option<String> {
        if src_url.starts_with("//") {
            return Some(format!("https:{}", src_url));
        }
        if src_url.starts_with("http://") || src_url.starts_with("https://") {
            return Some(src_url.to_string());
        }
        url::Url::parse(page_url).ok()
            .and_then(|base| base.join(src_url).ok())
            .map(|u| u.to_string())
    }

    pub fn execute_scripts(&self) -> Result<()> {
        let scripts = self.dom.borrow().extract_scripts();
        let page_url = self.context.with(|ctx| -> Result<String> {
            Ok(ctx.globals().get::<_, String>("__page_url")?)
        })?;

        const MAX_SCRIPTS: usize = 20;
        let scripts: Vec<_> = scripts.into_iter().take(MAX_SCRIPTS).collect();

        let mut to_fetch: Vec<(usize, String)> = Vec::new();
        for (i, (src, _)) in scripts.iter().enumerate() {
            if let Some(src_url) = src {
                if let Some(full_url) = Self::resolve_script_url(&page_url, src_url) {
                    to_fetch.push((i, full_url));
                }
            }
        }

        let fetch_client = reqwest::blocking::Client::builder()
            .user_agent("simpbro/0.1")
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| self.client.clone());

        let fetched: Vec<(usize, Option<String>)> = std::thread::scope(|s| {
            let handles: Vec<_> = to_fetch.iter().map(|(i, url)| {
                let client = &fetch_client;
                let url = url.clone();
                let idx = *i;
                s.spawn(move || {
                    let code = client.get(&url).send()
                        .and_then(|r| r.text())
                        .ok();
                    (idx, code)
                })
            }).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut fetched_map: std::collections::HashMap<usize, String> = fetched
            .into_iter()
            .filter_map(|(i, code)| code.map(|c| (i, c)))
            .collect();

        for (i, (src, inline)) in scripts.iter().enumerate() {
            let code = if src.is_some() {
                match fetched_map.remove(&i) {
                    Some(code) => code,
                    None => continue,
                }
            } else if let Some(code) = inline {
                code.clone()
            } else {
                continue;
            };

            self.context.with(|ctx| {
                if let Err(e) = ctx.eval::<Value, _>(code.as_str()) {
                    self.logs.borrow_mut().push(format!("[JS Error] {}", e));
                }
            });
        }

        self.run_immediate_timers();
        Ok(())
    }

    fn run_immediate_timers(&self) {
        let timers: Vec<TimerEntry> = self.timers.borrow_mut().drain(..).collect();
        for timer in timers {
            if timer.ms <= 0 {
                self.context.with(|ctx| {
                    let code = format!("if (window.__fire_timer) window.__fire_timer({})", timer.id);
                    let _ = ctx.eval::<Value, _>(code.as_str());
                });
            }
        }
    }

    pub fn dom(&self) -> std::cell::Ref<'_, Dom> {
        self.dom.borrow()
    }

    pub fn logs(&self) -> Vec<String> {
        self.logs.borrow().clone()
    }

    /// Check if JS changed window.location.href (e.g. via location.replace)
    pub fn redirected_url(&self) -> Option<String> {
        self.context.with(|ctx| -> Option<String> {
            let globals = ctx.globals();
            let original: String = globals.get("__page_url").ok()?;
            let current: String = ctx.eval::<String, _>("window.location.href").ok()?;
            if current != original && !current.is_empty() {
                Some(current)
            } else {
                None
            }
        })
    }
}
