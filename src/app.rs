use crate::browser::{Browser, Page, PageLine};
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
    pub focused: Option<usize>,
    pub viewport_height: Option<usize>,
    pub input: String,
    pub input_mode: InputMode,
    pub status_msg: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            browser: Browser::new()?,
            current_page: None,
            scroll_offset: 0,
            focused: None,
            viewport_height: None,
            input: String::new(),
            input_mode: InputMode::Normal,
            status_msg: "按 g 输入网址 | q 退出".into(),
            should_quit: false,
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
                self.focused = self.first_focusable(&page);
                self.current_page = Some(page);
            }
            Err(e) => {
                self.status_msg = format!("错误: {}", e);
            }
        }
    }

    fn is_focusable(line: &PageLine) -> bool {
        matches!(line, PageLine::LinkRef(..) | PageLine::InputRef(..) | PageLine::ButtonRef(..))
    }

    fn focusable_indices(&self) -> Vec<usize> {
        match &self.current_page {
            Some(page) => page.lines.iter().enumerate()
                .filter(|(_, l)| Self::is_focusable(l))
                .map(|(i, _)| i)
                .collect(),
            None => Vec::new(),
        }
    }

    fn first_focusable(&self, page: &Page) -> Option<usize> {
        page.lines.iter().position(|l| Self::is_focusable(l))
    }

    pub fn focus_next(&mut self) {
        let indices = self.focusable_indices();
        if indices.is_empty() { return; }
        self.focused = Some(match self.focused {
            Some(cur) => {
                indices.iter().find(|&&i| i > cur).copied()
                    .unwrap_or(indices[0])
            }
            None => indices[0],
        });
        self.scroll_to_focused();
    }

    pub fn focus_prev(&mut self) {
        let indices = self.focusable_indices();
        if indices.is_empty() { return; }
        self.focused = Some(match self.focused {
            Some(cur) => {
                indices.iter().rev().find(|&&i| i < cur).copied()
                    .unwrap_or(*indices.last().unwrap())
            }
            None => *indices.last().unwrap(),
        });
        self.scroll_to_focused();
    }

    fn scroll_to_focused(&mut self) {
        if let Some(f) = self.focused {
            if f < self.scroll_offset {
                self.scroll_offset = f;
            }
            // Can't know exact viewport height here, use a reasonable default
            // The actual viewport height is set during rendering; use 20 as fallback
            let viewport = self.viewport_height.unwrap_or(20);
            if f >= self.scroll_offset + viewport {
                self.scroll_offset = f - viewport + 1;
            }
        }
    }

    pub fn activate_focused(&mut self) {
        let focused = match self.focused {
            Some(f) => f,
            None => return,
        };
        // Extract what we need then dispatch
        let action = {
            let page = match &self.current_page {
                Some(p) => p,
                None => return,
            };
            page.lines.get(focused).cloned()
        };
        match action {
            Some(PageLine::LinkRef(_, idx, _)) => {
                let url_opt = self.current_page.as_mut()
                    .and_then(|p| p.click_link(idx));
                if let Some(url) = url_opt {
                    self.navigate(&url);
                }
            }
            Some(PageLine::InputRef(_, idx, _)) => {
                if let Some(page) = &self.current_page {
                    if let Some(inp) = page.inputs.get(idx) {
                        self.input = inp.value.clone();
                        self.input_mode = InputMode::FormInput(idx);
                        let hint = if inp.placeholder.is_empty() { &inp.name } else { &inp.placeholder };
                        self.status_msg = format!("编辑 {} (Enter 确认, Esc 取消)", hint);
                    }
                }
            }
            Some(PageLine::ButtonRef(_, idx, _)) => {
                let url_opt = self.current_page.as_mut()
                    .and_then(|p| p.click_button(idx));
                if let Some(url) = url_opt {
                    self.navigate(&url);
                }
            }
            _ => {}
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

    pub fn submit_form(&mut self) {
        let input_idx = match self.focused {
            Some(f) => match self.current_page.as_ref().and_then(|p| p.lines.get(f)) {
                Some(PageLine::InputRef(_, idx, _)) => Some(*idx),
                _ => None,
            },
            None => None,
        };
        if let Some(idx) = input_idx {
            let url_opt = self.current_page.as_mut()
                .and_then(|p| p.submit_form(idx));
            if let Some(url) = url_opt {
                self.navigate(&url);
                return;
            }
        }
        self.status_msg = "当前焦点不是输入框或没有关联表单".into();
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

    pub fn visible_lines(&self, height: usize) -> Vec<&PageLine> {
        match &self.current_page {
            Some(page) => page
                .lines
                .iter()
                .skip(self.scroll_offset)
                .take(height)
                .collect(),
            None => Vec::new(),
        }
    }
}
