use crate::app::{App, InputMode};
use crate::browser::{PageLine, TextStyle};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

fn apply_text_style(base: Style, ts: &TextStyle) -> Style {
    let mut s = base;
    if let Some((r, g, b)) = ts.color {
        s = s.fg(Color::Rgb(r, g, b));
    }
    let mut m = Modifier::empty();
    if ts.bold { m |= Modifier::BOLD; }
    if ts.italic { m |= Modifier::ITALIC; }
    if ts.underline { m |= Modifier::UNDERLINED; }
    if ts.strikethrough { m |= Modifier::CROSSED_OUT; }
    if !m.is_empty() {
        s = s.add_modifier(m);
    }
    s
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    let url_text = match &app.input_mode {
        InputMode::UrlInput => app.input.as_str(),
        _ => app.current_page.as_ref().map(|p| p.url.as_str()).unwrap_or(""),
    };

    let url_style = match app.input_mode {
        InputMode::UrlInput => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::Cyan),
    };

    let url_bar = Paragraph::new(url_text)
        .style(url_style)
        .block(Block::default().borders(Borders::ALL).title(" simpbro "));
    f.render_widget(url_bar, chunks[0]);

    if matches!(app.input_mode, InputMode::UrlInput) {
        f.set_cursor_position((
            chunks[0].x + app.input.chars().count() as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    let content_height = chunks[1].height as usize;
    app.viewport_height = Some(content_height);
    let visible = app.visible_lines(content_height);

    let content_lines: Vec<Line> = if visible.is_empty() {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  simpbro - 终端浏览器",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("  按 g 输入网址"),
            Line::from("  按 q 退出"),
            Line::from(""),
            Line::from("  导航:"),
            Line::from("    j/k 或 ↑/↓    滚动"),
            Line::from("    Tab/Shift+Tab  切换焦点"),
            Line::from("    Enter          打开链接/编辑输入框"),
            Line::from("    s              提交表单"),
            Line::from("    b              后退"),
        ]
    } else {
        visible
            .iter()
            .enumerate()
            .map(|(vi, line)| {
                let line_idx = app.scroll_offset + vi;
                let is_focused = app.focused == Some(line_idx);

                match line {
                    PageLine::Heading(text, level, ts) => {
                        let base = match level {
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
                        Line::from(Span::styled(text.to_string(), apply_text_style(base, ts)))
                    }
                    PageLine::Text(text, ts) => {
                        if ts.color.is_some() || ts.bold || ts.italic || ts.underline || ts.strikethrough {
                            Line::from(Span::styled(text.to_string(), apply_text_style(Style::default(), ts)))
                        } else {
                            Line::from(text.as_str())
                        }
                    }
                    PageLine::LinkRef(text, _, ts) => {
                        let base = if is_focused {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::UNDERLINED)
                        };
                        // For focused links keep the highlight colors; for unfocused, let CSS color override
                        let style = if is_focused { base } else { apply_text_style(base, ts) };
                        Line::from(Span::styled(text.to_string(), style))
                    }
                    PageLine::InputRef(placeholder, idx, _ts) => {
                        let value = app.current_page.as_ref()
                            .and_then(|p| p.inputs.get(*idx))
                            .map(|inp| inp.value.as_str())
                            .unwrap_or("");
                        let is_editing = matches!(&app.input_mode, InputMode::FormInput(i) if *i == *idx);

                        let display = if is_editing {
                            format!("[{}|]", app.input)
                        } else if value.is_empty() {
                            format!("[{}]", placeholder)
                        } else {
                            format!("[{}]", value)
                        };

                        let style = if is_editing {
                            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                        } else if is_focused {
                            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Green)
                        };
                        Line::from(Span::styled(display, style))
                    }
                    PageLine::ButtonRef(label, _, _ts) => {
                        let display = format!("[{}]", label);
                        let style = if is_focused {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Magenta)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD)
                        };
                        Line::from(Span::styled(display, style))
                    }
                    PageLine::Blank => Line::from(""),
                }
            })
            .collect()
    };

    let content = Paragraph::new(content_lines)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT));
    f.render_widget(content, chunks[1]);

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            &app.status_msg,
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
    ]))
    .style(Style::default().bg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}
