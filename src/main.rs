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
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
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

    // Cached info from the last draw, used to interpret mouse events.
    let mut last_content: Rect = Rect::default();
    let mut last_viewport: Option<viewport::Viewport> = None;

    loop {
        terminal.draw(|f| {
            let res = ui::draw(f, &mut app);
            last_content = res.content_area;
            last_viewport = Some(res.viewport);
        })?;

        let ev = event::read()?;

        if let Event::Mouse(mouse_ev) = &ev {
            let vp = match &last_viewport {
                Some(v) => v,
                None => continue,
            };
            let page_ref = app.current_page.as_ref();
            let action = app.mouse.handle(mouse_ev, last_content, vp, page_ref);
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
            }
            continue;
        }

        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press { continue; }

            // Global: Ctrl+C exits, Ctrl+Z goes back.
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('c') => { app.should_quit = true; break; }
                    KeyCode::Char('z') => { app.go_back(); continue; }
                    _ => {}
                }
            }

            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('g') => {
                        app.input_mode = InputMode::UrlInput;
                        app.input.clear();
                        app.status_msg = "输入网址 (Esc 取消)".into();
                    }
                    KeyCode::Down => app.scroll_down(1),
                    KeyCode::Up => app.scroll_up(1),
                    KeyCode::Tab => app.focus_next(),
                    KeyCode::BackTab => app.focus_prev(),
                    KeyCode::Enter => app.activate_focused(),
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
                        app.status_msg = "按 g 输入网址 | Ctrl+C 退出".into();
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => { app.input.pop(); }
                    _ => {}
                },
                InputMode::FormInput(_) => match key.code {
                    KeyCode::Enter => app.confirm_form_input(),
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.status_msg = "按 g 输入网址 | Ctrl+C 退出".into();
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
