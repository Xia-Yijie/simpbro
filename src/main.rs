mod app;
mod browser;
mod css;
mod dom;
mod js_engine;
mod ui;

use anyhow::Result;
use app::{App, InputMode};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
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

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let ev = event::read()?;

        if let Event::Mouse(mouse) = &ev {
            match mouse.kind {
                MouseEventKind::ScrollUp => { app.scroll_up(3); continue; }
                MouseEventKind::ScrollDown => { app.scroll_down(3); continue; }
                MouseEventKind::ScrollLeft => { app.focus_prev(); continue; }
                MouseEventKind::ScrollRight => { app.focus_next(); continue; }
                _ => continue, // ignore mouse move/click without redraw
            }
        }

        if let Event::Key(key) = ev {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    KeyCode::Char('g') => {
                        app.input_mode = InputMode::UrlInput;
                        app.input.clear();
                        app.status_msg = "输入网址 (Esc 取消)".into();
                    }
                    KeyCode::Char('s') => app.submit_form(),
                    KeyCode::Char('j') | KeyCode::Down => app.scroll_down(1),
                    KeyCode::Char('k') | KeyCode::Up => app.scroll_up(1),
                    KeyCode::Char('d') => app.scroll_down(10),
                    KeyCode::Char('u') => app.scroll_up(10),
                    KeyCode::Tab => app.focus_next(),
                    KeyCode::BackTab => app.focus_prev(),
                    KeyCode::Enter => app.activate_focused(),
                    KeyCode::Char('b') => app.go_back(),
                    _ => {}
                },
                InputMode::UrlInput => match key.code {
                    KeyCode::Enter => {
                        let url = app.input.clone();
                        app.input_mode = InputMode::Normal;
                        if !url.is_empty() {
                            app.navigate(&url);
                        }
                    }
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.status_msg = "按 g 输入网址 | q 退出".into();
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => { app.input.pop(); }
                    _ => {}
                },
                InputMode::FormInput(_) => match key.code {
                    KeyCode::Enter => app.confirm_form_input(),
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.status_msg = "按 g 输入网址 | q 退出".into();
                    }
                    KeyCode::Char(c) => app.input.push(c),
                    KeyCode::Backspace => { app.input.pop(); }
                    _ => {}
                },
            }

            if app.should_quit {
                break;
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
