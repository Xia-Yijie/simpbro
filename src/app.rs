use crate::browser::{Browser, FocusKind, Page};
use crate::mouse::MouseState;
use anyhow::Result;

pub enum InputMode {
    Normal,
    UrlInput,
    FormInput(usize), // editing input at index
}

pub struct App {
    pub browser: Browser,
    pub current_page: Option<Page>,
    pub scroll_offset: usize,
    /// Index into page.focus_order.
    pub focused: Option<usize>,
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
        let welcome = browser.load_embedded(welcome_html, "about:simpbro").ok();
        let focused = welcome.as_ref().and_then(|p| if p.focus_order.is_empty() { None } else { Some(0) });
        Ok(Self {
            browser,
            current_page: welcome,
            scroll_offset: 0,
            focused,
            viewport_height: None,
            input: String::new(),
            input_mode: InputMode::Normal,
            status_msg: "按 g 输入网址 | Ctrl+C 退出".into(),
            should_quit: false,
            mouse: MouseState::default(),
        })
    }

    pub fn navigate(&mut self, url: &str) {
        self.status_msg = format!("加载中 {}...", url);
        let result = self.browser.fetch(url);
        self.apply_page(result);
    }

    fn apply_page(&mut self, result: Result<Page>) {
        match result {
            Ok(page) => {
                self.status_msg = format!("已加载: {}", page.title);
                self.scroll_offset = 0;
                self.focused = if page.focus_order.is_empty() { None } else { Some(0) };
                self.current_page = Some(page);
            }
            Err(e) => {
                self.status_msg = format!("错误: {}", e);
            }
        }
    }

    fn focus_count(&self) -> usize {
        self.current_page.as_ref().map(|p| p.focus_order.len()).unwrap_or(0)
    }

    pub fn set_focus(&mut self, idx: usize) {
        let n = self.focus_count();
        if idx < n {
            self.focused = Some(idx);
            self.scroll_to_focused();
        }
    }

    pub fn focus_next(&mut self) {
        let n = self.focus_count();
        if n == 0 { return; }
        self.focused = Some(match self.focused {
            Some(i) => (i + 1) % n,
            None => 0,
        });
        self.scroll_to_focused();
    }

    pub fn focus_prev(&mut self) {
        let n = self.focus_count();
        if n == 0 { return; }
        self.focused = Some(match self.focused {
            Some(0) | None => n - 1,
            Some(i) => i - 1,
        });
        self.scroll_to_focused();
    }

    fn focused_line(&self) -> Option<usize> {
        let f = self.focused?;
        self.current_page.as_ref()?.focus_order.get(f).map(|fi| fi.line)
    }

    fn scroll_to_focused(&mut self) {
        if let Some(line) = self.focused_line() {
            if line < self.scroll_offset {
                self.scroll_offset = line;
            }
            let viewport = self.viewport_height.unwrap_or(20);
            if line >= self.scroll_offset + viewport {
                self.scroll_offset = line - viewport + 1;
            }
        }
    }

    pub fn activate_focused(&mut self) {
        let kind = match self.focused.and_then(|f| self.current_page.as_ref()?.focus_order.get(f).map(|fi| fi.kind)) {
            Some(k) => k,
            None => return,
        };
        match kind {
            FocusKind::Link(idx) => {
                let url_opt = self.current_page.as_mut().and_then(|p| p.click_link(idx));
                if let Some(url) = url_opt {
                    self.navigate(&url);
                }
            }
            FocusKind::Input(idx) => {
                if let Some(page) = &self.current_page {
                    if let Some(inp) = page.inputs.get(idx) {
                        self.input = inp.value.clone();
                        self.input_mode = InputMode::FormInput(idx);
                        let hint = if inp.placeholder.is_empty() { &inp.name } else { &inp.placeholder };
                        self.status_msg = format!("编辑 {} (Enter 确认, Esc 取消)", hint);
                    }
                }
            }
            FocusKind::Button(idx) => {
                let url_opt = self.current_page.as_mut().and_then(|p| p.click_button(idx));
                if let Some(url) = url_opt {
                    self.navigate(&url);
                }
            }
        }
    }

    pub fn confirm_form_input(&mut self) {
        if let InputMode::FormInput(idx) = self.input_mode {
            let value = self.input.clone();
            if let Some(page) = &mut self.current_page {
                page.set_input_value(idx, value.clone());
            }
            self.status_msg = format!("已输入: {}", value);
            self.input_mode = InputMode::Normal;
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        if let Some(page) = &self.current_page {
            let max = page.lines.len().saturating_sub(1);
            self.scroll_offset = (self.scroll_offset + amount).min(max);
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn go_back(&mut self) {
        let result = self.browser.go_back();
        if let Some(result) = result {
            self.apply_page(result);
        } else {
            self.status_msg = "没有历史记录".into();
        }
    }

}
