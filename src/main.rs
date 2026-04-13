mod app;
mod browser;
mod css;
mod dom;
mod js_engine;
mod mouse;
mod ui;
mod viewport;

use anyhow::Result;
use app::{App, InputMode};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mouse::MouseAction;
use ratatui::{
    layout::Rect,
    prelude::*,
};
use std::io;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    if let Some(url) = std::env::args().nth(1) {
        app.navigate(&url);
    }

    let mut last_content: Rect = Rect::default();
    let mut last_tab_bar: Rect = Rect::default();
    let mut last_url_bar: Rect = Rect::default();
    let mut last_tab_regions: Vec<(u16, u16)> = Vec::new();
    let mut last_back: Option<(u16, u16)> = None;
    let mut last_refresh: Option<(u16, u16)> = None;
    let mut last_viewport: Option<viewport::Viewport> = None;

    loop {
        terminal.draw(|f| {
            let res = ui::draw(f, &mut app);
            last_content = res.content_area;
            last_tab_bar = res.tab_bar_area;
            last_url_bar = res.url_bar_area;
            last_tab_regions = res.tab_regions;
            last_back = res.back_region;
            last_refresh = res.refresh_region;
            last_viewport = Some(res.viewport);
        })?;

        let ev = event::read()?;

        if let Event::Mouse(mouse_ev) = &ev {
            let vp = match &last_viewport {
                Some(v) => v,
                None => continue,
            };

            // Left-click updates focus zone based on which area was hit, and
            // routes back/refresh button clicks without going through mouse.rs.
            if let MouseEventKind::Down(MouseButton::Left) = mouse_ev.kind {
                let (c, r) = (mouse_ev.column, mouse_ev.row);
                if hit(last_back, c, r, last_tab_bar) {
                    app.go_back();
                    continue;
                }
                if hit(last_refresh, c, r, last_tab_bar) {
                    app.refresh();
                    continue;
                }
                let on_tab_label = last_tab_regions.iter().any(|(lo, hi)| c >= *lo && c <= *hi && mouse::in_rect(last_tab_bar, c, r));
                if mouse::in_rect(last_tab_bar, c, r) && !on_tab_label {
                    app.input_mode = InputMode::TabNav;
                    app.status_msg = "操作栏: Tab/Shift+Tab 切换标签，Esc 返回".into();
                } else if mouse::in_rect(last_url_bar, c, r) {
                    app.input.clear();
                    app.input_mode = InputMode::UrlInput;
                    app.status_msg = "输入网址 (Esc 取消)".into();
                } else if mouse::in_rect(last_content, c, r)
                    && matches!(app.input_mode, InputMode::TabNav | InputMode::UrlInput)
                {
                    app.input_mode = InputMode::Normal;
                    app.status_msg = app::STATUS_DEFAULT.into();
                }
            }

            let page_ref = app.tabs[app.active_tab].page.as_ref();
            let action = app.mouse.handle(mouse_ev, last_content, last_tab_bar, &last_tab_regions, vp, page_ref);
            match action {
                MouseAction::None => {}
                MouseAction::ScrollUp(n) => app.scroll_up(n),
                MouseAction::ScrollDown(n) => app.scroll_down(n),
                MouseAction::FocusAt(idx) => app.set_focus(idx),
                MouseAction::ActivateAt(idx) => {
                    app.set_focus(idx);
                    app.activate_focused();
                }
                MouseAction::Copy(text) => {
                    if mouse::osc52_copy(&text).is_ok() {
                        let preview: String = text.chars().take(30).collect();
                        app.status_msg = format!("已复制: {}", preview);
                    } else {
                        app.status_msg = "复制失败".into();
                    }
                }
                MouseAction::SwitchTab(idx) => app.switch_tab(idx),
            }
            continue;
        }

        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press { continue; }

            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('c') => { app.should_quit = true; break; }
                    KeyCode::Char('z') => { app.go_back(); continue; }
                    KeyCode::Char('t') => { app.new_tab(); continue; }
                    KeyCode::Char('w') => {
                        app.close_tab();
                        if app.should_quit { break; }
                        continue;
                    }
                    KeyCode::Tab => { app.next_tab(); continue; }
                    KeyCode::BackTab => { app.prev_tab(); continue; }
                    KeyCode::Char('g') => {
                        // Ctrl+G switches focus between bars (操作栏 ⇌ 地址栏).
                        // From Normal/FormInput, enters 操作栏. Esc returns to content.
                        app.input_mode = match app.input_mode {
                            InputMode::UrlInput => {
                                app.status_msg = "操作栏: Tab/Shift+Tab 切换标签，Esc 返回".into();
                                InputMode::TabNav
                            }
                            _ => {
                                app.input.clear();
                                app.status_msg = "输入网址 (Esc 取消)".into();
                                InputMode::UrlInput
                            }
                        };
                        continue;
                    }
                    _ => {}
                }
            }

            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Down => app.scroll_down(1),
                    KeyCode::Up => app.scroll_up(1),
                    KeyCode::Tab => app.focus_next(),
                    KeyCode::BackTab => app.focus_prev(),
                    KeyCode::Enter => app.activate_focused(),
                    _ => {}
                },
                InputMode::TabNav => match key.code {
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::BackTab => app.prev_tab(),
                    KeyCode::Esc | KeyCode::Enter => {
                        app.input_mode = InputMode::Normal;
                        app.status_msg = app::STATUS_DEFAULT.into();
                    }
                    _ => {}
                },
                InputMode::UrlInput => match key.code {
                    KeyCode::Enter => {
                        let url = app.input.clone();
                        app.input_mode = InputMode::Normal;
                        if !url.is_empty() { app.navigate(&url); }
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.status_msg = app::STATUS_DEFAULT.into();
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => { app.input.pop(); }
                    _ => {}
                },
                InputMode::FormInput(_) => match key.code {
                    KeyCode::Enter => app.confirm_form_input(),
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.status_msg = app::STATUS_DEFAULT.into();
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => { app.input.pop(); }
                    _ => {}
                },
            }

            if app.should_quit { break; }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}

fn hit(region: Option<(u16, u16)>, col: u16, row: u16, bar: Rect) -> bool {
    match region {
        Some((lo, hi)) => mouse::in_rect(bar, col, row) && col >= lo && col <= hi,
        None => false,
    }
}
