mod app;
mod browser;
mod dom;
mod js_engine;
mod ui;

use anyhow::Result;
use app::{App, InputMode};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    // Optional: navigate to a URL passed as argument
    if let Some(url) = std::env::args().nth(1) {
        app.navigate(&url);
    }

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Event::Key(key) = event::read()? {
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
                        app.status_msg = "Enter URL (Esc to cancel)".into();
                    }
                    KeyCode::Char('j') | KeyCode::Down => app.scroll_down(1),
                    KeyCode::Char('k') | KeyCode::Up => app.scroll_up(1),
                    KeyCode::Char('d') => app.scroll_down(10),
                    KeyCode::Char('u') => app.scroll_up(10),
                    KeyCode::Tab => app.next_link(),
                    KeyCode::BackTab => app.prev_link(),
                    KeyCode::Enter => app.follow_selected_link(),
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
                        app.status_msg = "Press 'g' to enter URL, 'q' to quit".into();
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    _ => {}
                },
            }

            if app.should_quit {
                break;
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
