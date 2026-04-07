use crate::browser::{Browser, Page, PageLine};
use anyhow::Result;

pub enum InputMode {
    Normal,
    UrlInput,
}

pub struct App {
    pub browser: Browser,
    pub current_page: Option<Page>,
    pub scroll_offset: usize,
    pub selected_link: Option<usize>,
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
            selected_link: None,
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
                self.selected_link = if page.links.is_empty() { None } else { Some(0) };
                self.current_page = Some(page);
            }
            Err(e) => {
                self.status_msg = format!("错误: {}", e);
            }
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

    pub fn next_link(&mut self) {
        if let Some(page) = &self.current_page {
            if page.links.is_empty() {
                return;
            }
            self.selected_link = Some(match self.selected_link {
                Some(i) => (i + 1) % page.links.len(),
                None => 0,
            });
        }
    }

    pub fn prev_link(&mut self) {
        if let Some(page) = &self.current_page {
            if page.links.is_empty() {
                return;
            }
            self.selected_link = Some(match self.selected_link {
                Some(0) | None => page.links.len().saturating_sub(1),
                Some(i) => i - 1,
            });
        }
    }

    pub fn follow_selected_link(&mut self) {
        if let (Some(page), Some(idx)) = (&self.current_page, self.selected_link) {
            if let Some(link) = page.links.get(idx) {
                let url = link.url.clone();
                self.navigate(&url);
            }
        }
    }

    pub fn go_back(&mut self) {
        let result = self.browser.go_back();
        if let Some(result) = result {
            self.apply_page(result);
        } else {
            self.status_msg = "没有历史记录".into();
        }
    }

    /// Get visible lines for rendering
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
