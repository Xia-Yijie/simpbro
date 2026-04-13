use crate::app::{App, InputMode};
use crate::browser::{FocusKind, TextStyle};
use crate::viewport::{Cell, Viewport};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
    if !m.is_empty() { s = s.add_modifier(m); }
    s
}

/// Result of drawing: content area Rect + computed Viewport (for mouse mapping).
pub struct DrawResult {
    pub content_area: Rect,
    pub viewport: Viewport,
}

pub fn draw(f: &mut Frame, app: &mut App) -> DrawResult {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());

    // ---- URL bar ----
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
        .block(Block::default().borders(Borders::ALL).title(" 地址栏 "));
    f.render_widget(url_bar, chunks[0]);
    if matches!(app.input_mode, InputMode::UrlInput) {
        f.set_cursor_position((
            chunks[0].x + app.input.chars().count() as u16 + 1,
            chunks[0].y + 1,
        ));
    }

    // ---- Content: border box + inner grid ----
    let outer = chunks[1];
    let block = Block::default().borders(Borders::LEFT | Borders::RIGHT);
    let inner = block.inner(outer);
    f.render_widget(block, outer);

    app.viewport_height = Some(inner.height as usize);

    // Build viewport grid from page model.
    let input_override = match app.input_mode {
        InputMode::FormInput(idx) => Some((idx, app.input.as_str())),
        _ => None,
    };
    let viewport = Viewport::build(
        app.current_page.as_ref(),
        app.scroll_offset,
        inner.width,
        inner.height,
        input_override,
    );

    // Focus info for highlight
    let focused_kind: Option<FocusKind> = app.focused
        .and_then(|f| app.current_page.as_ref()?.focus_order.get(f).map(|fi| fi.kind));

    // Selection info (in content-local coords)
    let selection = app.mouse.selection();

    let lines: Vec<Line> = viewport.rows.iter().enumerate().map(|(row_idx, row)| {
        row_to_line(row, row_idx, focused_kind, selection, &app.input_mode)
    }).collect();

    let content = Paragraph::new(lines);
    f.render_widget(content, inner);

    // ---- Status bar ----
    let status = Paragraph::new(app.status_msg.as_str())
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL).title(" 状态栏 "));
    f.render_widget(status, chunks[2]);

    DrawResult { content_area: inner, viewport }
}

fn row_to_line(
    row: &crate::viewport::Row,
    row_idx: usize,
    focused_kind: Option<FocusKind>,
    selection: Option<((u16, u16), (u16, u16))>,
    input_mode: &InputMode,
) -> Line<'static> {
    let sel = selection.map(|(a, b)| {
        if (a.1, a.0) <= (b.1, b.0) { (a, b) } else { (b, a) }
    });

    // Run-length merge adjacent cells sharing the same style into one Span.
    let mut spans: Vec<Span> = Vec::new();
    let mut run_text = String::new();
    let mut run_style: Option<Style> = None;
    let mut col: u16 = 0;

    let flush = |spans: &mut Vec<Span>, run_text: &mut String, run_style: &mut Option<Style>| {
        if let Some(s) = run_style.take() {
            spans.push(Span::styled(std::mem::take(run_text), s));
        }
    };

    for cell in &row.cells {
        let mut style = cell_style(cell, focused_kind, input_mode);
        if in_selection(sel, col, row_idx as u16) {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let ch = if cell.is_padding { ' ' } else { cell.ch };

        if run_style == Some(style) {
            run_text.push(ch);
        } else {
            flush(&mut spans, &mut run_text, &mut run_style);
            run_text.push(ch);
            run_style = Some(style);
        }
        col = col.saturating_add(cell.width as u16);
    }
    flush(&mut spans, &mut run_text, &mut run_style);

    Line::from(spans)
}

fn cell_style(cell: &Cell, focused_kind: Option<FocusKind>, input_mode: &InputMode) -> Style {
    match cell.focus {
        Some(FocusKind::Link(_)) => {
            if focused_kind == cell.focus {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                apply_text_style(
                    Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
                    &cell.style,
                )
            }
        }
        Some(FocusKind::Input(idx)) => {
            let is_editing = matches!(input_mode, InputMode::FormInput(i) if *i == idx);
            if is_editing {
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if focused_kind == cell.focus {
                Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            }
        }
        Some(FocusKind::Button(_)) => {
            if focused_kind == cell.focus {
                Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
            }
        }
        None => apply_text_style(Style::default(), &cell.style),
    }
}

fn in_selection(sel: Option<((u16, u16), (u16, u16))>, col: u16, row: u16) -> bool {
    let ((a_col, a_row), (b_col, b_row)) = match sel {
        Some(s) => s,
        None => return false,
    };
    if row < a_row || row > b_row { return false; }
    let (lo, hi) = if a_row == b_row {
        (a_col.min(b_col), a_col.max(b_col))
    } else if row == a_row {
        (a_col, u16::MAX)
    } else if row == b_row {
        (0, b_col)
    } else {
        (0, u16::MAX)
    };
    col >= lo && col <= hi
}
