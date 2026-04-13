use crate::browser::{Browser, FocusKind, Page};
use crate::mouse::MouseState;
use anyhow::Result;

pub const STATUS_DEFAULT: &str = "Ctrl+G 切换焦点 | Ctrl+T 新标签 | Ctrl+C 退出";

pub enum InputMode {
    Normal,
    TabNav,
    UrlInput,
    FormInput(usize),
}

/// Per-tab state: its page, navigation history, viewport position, focus.
pub struct Tab {
    pub page: Option<Page>,
    pub history: Vec<String>,
    pub scroll_offset: usize,
    pub focused: Option<usize>,
}

impl Tab {
    fn new(page: Option<Page>) -> Self {
        let focused = page.as_ref().and_then(|p| if p.focus_order.is_empty() { None } else { Some(0) });
        let history = page.as_ref().map(|p| vec![p.url.clone()]).unwrap_or_default();
        Self { page, history, scroll_offset: 0, focused }
    }

    pub fn title(&self) -> String {
        match &self.page {
            Some(p) if !p.title.is_empty() => p.title.clone(),
            Some(p) => p.url.clone(),
            None => "(空)".to_string(),
        }
    }
}

pub struct App {
    pub browser: Browser,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub viewport_height: Option<usize>,
    pub input: String,
    pub input_mode: InputMode,
    pub status_msg: String,
    pub should_quit: bool,
    pub mouse: MouseState,
}

impl App {
    pub fn new() -> Result<Self> {
        let browser = Browser::new()?;
        let welcome_html = include_str!("welcome.html");
        let welcome = browser.load_embedded(welcome_html, "simpbro://about").ok();
        Ok(Self {
            browser,
            tabs: vec![Tab::new(welcome)],
            active_tab: 0,
            viewport_height: None,
            input: String::new(),
            input_mode: InputMode::Normal,
            status_msg: STATUS_DEFAULT.into(),
            should_quit: false,
            mouse: MouseState::default(),
        })
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    pub fn current_page(&self) -> Option<&Page> {
        self.tab().page.as_ref()
    }

    pub fn navigate(&mut self, url: &str) {
        self.status_msg = format!("加载中 {}...", url);
        let result = self.browser.fetch(url);
        self.apply_page(result);
    }

    fn apply_page(&mut self, result: Result<Page>) {
        self.apply_page_inner(result, /* push_history */ true);
    }

    fn apply_page_inner(&mut self, result: Result<Page>, push_history: bool) {
        match result {
            Ok(page) => {
                self.status_msg = format!("已加载: {}", page.title);
                let t = self.tab_mut();
                t.scroll_offset = 0;
                t.focused = if page.focus_order.is_empty() { None } else { Some(0) };
                if push_history {
                    if t.history.len() >= 100 { t.history.remove(0); }
                    t.history.push(page.url.clone());
                }
                t.page = Some(page);
            }
            Err(e) => {
                self.status_msg = format!("错误: {}", e);
            }
        }
    }

    // ---- tab operations ----

    pub fn new_tab(&mut self) {
        let html = include_str!("welcome.html");
        let page = self.browser.load_embedded(html, "simpbro://about").ok();
        self.tabs.push(Tab::new(page));
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self) {
        if self.tabs.len() == 1 {
            self.should_quit = true;
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = if self.active_tab == 0 { self.tabs.len() - 1 } else { self.active_tab - 1 };
        }
    }

    pub fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
        }
    }

    // ---- focus + scroll ----

    fn focus_count(&self) -> usize {
        self.current_page().map(|p| p.focus_order.len()).unwrap_or(0)
    }

    pub fn set_focus(&mut self, idx: usize) {
        let n = self.focus_count();
        if idx < n {
            self.tab_mut().focused = Some(idx);
            self.scroll_to_focused();
        }
    }

    pub fn focus_next(&mut self) {
        let n = self.focus_count();
        if n == 0 { return; }
        let next = match self.tab().focused {
            Some(i) => (i + 1) % n,
            None => 0,
        };
        self.tab_mut().focused = Some(next);
        self.scroll_to_focused();
    }

    pub fn focus_prev(&mut self) {
        let n = self.focus_count();
        if n == 0 { return; }
        let prev = match self.tab().focused {
            Some(0) | None => n - 1,
            Some(i) => i - 1,
        };
        self.tab_mut().focused = Some(prev);
        self.scroll_to_focused();
    }

    fn focused_line(&self) -> Option<usize> {
        let t = self.tab();
        let f = t.focused?;
        t.page.as_ref()?.focus_order.get(f).map(|fi| fi.line)
    }

    fn scroll_to_focused(&mut self) {
        let line = match self.focused_line() { Some(l) => l, None => return };
        let viewport = self.viewport_height.unwrap_or(20);
        let t = self.tab_mut();
        if line < t.scroll_offset {
            t.scroll_offset = line;
        }
        if line >= t.scroll_offset + viewport {
            t.scroll_offset = line - viewport + 1;
        }
    }

    pub fn activate_focused(&mut self) {
        let kind = match self.tab().focused
            .and_then(|f| self.tab().page.as_ref()?.focus_order.get(f).map(|fi| fi.kind))
        {
            Some(k) => k,
            None => return,
        };
        match kind {
            FocusKind::Link(idx) => {
                let url_opt = self.tab_mut().page.as_mut().and_then(|p| p.click_link(idx));
                if let Some(url) = url_opt { self.navigate(&url); }
            }
            FocusKind::Input(idx) => {
                let inp = self.current_page().and_then(|p| p.inputs.get(idx)).cloned();
                if let Some(inp) = inp {
                    self.input = inp.value.clone();
                    self.input_mode = InputMode::FormInput(idx);
                    let hint = if inp.placeholder.is_empty() { &inp.name } else { &inp.placeholder };
                    self.status_msg = format!("编辑 {} (Enter 确认, Esc 取消)", hint);
                }
            }
            FocusKind::Button(idx) => {
                let url_opt = self.tab_mut().page.as_mut().and_then(|p| p.click_button(idx));
                if let Some(url) = url_opt { self.navigate(&url); }
            }
        }
    }

    pub fn confirm_form_input(&mut self) {
        if let InputMode::FormInput(idx) = self.input_mode {
            let value = self.input.clone();
            if let Some(page) = self.tab_mut().page.as_mut() {
                page.set_input_value(idx, value.clone());
            }
            self.status_msg = format!("已输入: {}", value);
            self.input_mode = InputMode::Normal;
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max = self.current_page().map(|p| p.lines.len().saturating_sub(1)).unwrap_or(0);
        let t = self.tab_mut();
        t.scroll_offset = (t.scroll_offset + amount).min(max);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        let t = self.tab_mut();
        t.scroll_offset = t.scroll_offset.saturating_sub(amount);
    }

    pub fn refresh(&mut self) {
        let url = match self.current_page().map(|p| p.url.clone()) {
            Some(u) => u,
            None => return,
        };
        self.navigate(&url);
    }

    pub fn go_back(&mut self) {
        if self.tab().history.len() < 2 {
            self.status_msg = "没有历史记录".into();
            return;
        }
        self.tab_mut().history.pop();
        let prev = match self.tab().history.last() {
            Some(u) => u.clone(),
            None => return,
        };
        let result = self.browser.fetch(&prev);
        self.apply_page_inner(result, false);
    }
}
