use crate::app::{App, InputMode};
use crate::browser::PageLine;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // URL bar
            Constraint::Min(1),   // Content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // URL bar
    let url_text = match &app.input_mode {
        InputMode::UrlInput => app.input.as_str(),
        InputMode::Normal => app
            .current_page
            .as_ref()
            .map(|p| p.url.as_str())
            .unwrap_or(""),
    };

    let url_style = match app.input_mode {
        InputMode::UrlInput => Style::default().fg(Color::Yellow),
        InputMode::Normal => Style::default().fg(Color::Cyan),
    };

    let url_bar = Paragraph::new(url_text)
        .style(url_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" simpbro "),
        );
    f.render_widget(url_bar, chunks[0]);

    // Set cursor position in URL input mode
    if matches!(app.input_mode, InputMode::UrlInput) {
        f.set_cursor_position((
            chunks[0].x + app.input.chars().count() as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    // Content area
    let content_height = chunks[1].height as usize;
    let visible = app.visible_lines(content_height);

    let content_lines: Vec<Line> = if visible.is_empty() {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Welcome to simpbro!",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("  Press 'g' to enter a URL"),
            Line::from("  Press 'q' to quit"),
            Line::from(""),
            Line::from("  Navigation:"),
            Line::from("    j/k or ↑/↓  - scroll"),
            Line::from("    Tab/Shift+Tab - cycle links"),
            Line::from("    Enter        - follow link"),
            Line::from("    b            - go back"),
        ]
    } else {
        visible
            .iter()
            .map(|line| match line {
                PageLine::Heading(text, level) => {
                    let style = match level {
                        1 => Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                        2 => Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                        _ => Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    };
                    Line::from(Span::styled(format!(" {}", text), style))
                }
                PageLine::Text(text) => Line::from(format!(" {}", text)),
                PageLine::LinkRef(text, idx) => {
                    let is_selected = app.selected_link == Some(*idx);
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::UNDERLINED)
                    };
                    Line::from(Span::styled(format!(" [{}] {}", idx, text), style))
                }
                PageLine::Blank => Line::from(""),
            })
            .collect()
    };

    let content = Paragraph::new(content_lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
    f.render_widget(content, chunks[1]);

    // Status bar
    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            &app.status_msg,
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
    ]))
    .style(Style::default().bg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}
