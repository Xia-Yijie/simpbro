// Character-grid layout for the content area.
//
// We do our own text wrapping (instead of using ratatui's `Wrap`) so we have
// a stable (row, col) → cell mapping for mouse interaction and selection.

use unicode_width::UnicodeWidthChar;

use crate::browser::{FocusKind, Page, PageLine, TextStyle};

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    /// Cell width in terminal columns: 1 (ASCII / narrow) or 2 (CJK / emoji).
    pub width: u8,
    pub style: TextStyle,
    /// Click on this cell → focus this interactive item.
    pub focus: Option<FocusKind>,
    /// True if this is a padding cell added to fill a row.
    pub is_padding: bool,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
}

pub struct Viewport {
    pub width: u16,
    pub rows: Vec<Row>,
}

impl Viewport {
    pub fn build(
        page: Option<&Page>,
        scroll_offset: usize,
        width: u16,
        height: u16,
        input_override: Option<(usize, &str)>,
    ) -> Self {
        let mut rows: Vec<Row> = Vec::new();
        if width == 0 { return Viewport { width, rows }; }
        if let Some(page) = page {
            for line in page.lines.iter().skip(scroll_offset) {
                if rows.len() >= height as usize { break; }
                emit_line(line, width, page, input_override, &mut rows);
            }
        }
        while rows.len() < height as usize {
            finish_row(&mut Vec::new(), width, &mut rows);
        }
        Viewport { width, rows }
    }

    /// Extract text between two (col, row) positions in reading order.
    pub fn extract_text(&self, mut a: (u16, u16), mut b: (u16, u16)) -> String {
        if (a.1, a.0) > (b.1, b.0) {
            std::mem::swap(&mut a, &mut b);
        }
        let mut out = String::new();
        for (row_idx, row) in self.rows.iter().enumerate() {
            let r = row_idx as u16;
            if r < a.1 || r > b.1 { continue; }
            let (col_start, col_end) = if a.1 == b.1 {
                (a.0.min(b.0), a.0.max(b.0))
            } else if r == a.1 {
                (a.0, self.width.saturating_sub(1))
            } else if r == b.1 {
                (0, b.0)
            } else {
                (0, self.width.saturating_sub(1))
            };

            let mut col: u16 = 0;
            let mut row_has_text = false;
            for cell in &row.cells {
                if col > col_end { break; }
                if col >= col_start && !cell.is_padding {
                    out.push(cell.ch);
                    row_has_text = true;
                }
                col = col.saturating_add(cell.width as u16);
            }
            if r != b.1 && row_has_text {
                out.push('\n');
            }
        }
        out.trim_end().to_string()
    }

    pub fn focus_at(&self, col: u16, row: u16) -> Option<FocusKind> {
        let row = self.rows.get(row as usize)?;
        let mut cur: u16 = 0;
        for cell in &row.cells {
            if cur <= col && col < cur + cell.width as u16 {
                return cell.focus;
            }
            cur += cell.width as u16;
        }
        None
    }
}

fn padding_cell() -> Cell {
    Cell {
        ch: ' ',
        width: 1,
        style: TextStyle::default(),
        focus: None,
        is_padding: true,
    }
}

fn finish_row(buf: &mut Vec<Cell>, width: u16, rows: &mut Vec<Row>) {
    while cells_width(buf) < width as usize {
        buf.push(padding_cell());
    }
    rows.push(Row { cells: std::mem::take(buf) });
}

fn cells_width(cells: &[Cell]) -> usize {
    cells.iter().map(|c| c.width as usize).sum()
}

fn push_char(buf: &mut Vec<Cell>, ch: char, style: TextStyle, focus: Option<FocusKind>) {
    let w = UnicodeWidthChar::width(ch).unwrap_or(1).clamp(1, 2) as u8;
    buf.push(Cell { ch, width: w, style, focus, is_padding: false });
}

fn emit_line(
    line: &PageLine,
    width: u16,
    page: &Page,
    input_override: Option<(usize, &str)>,
    rows: &mut Vec<Row>,
) {
    let mut buf: Vec<Cell> = Vec::new();

    let push = |buf: &mut Vec<Cell>, rows: &mut Vec<Row>, text: &str, style: TextStyle, focus: Option<FocusKind>| {
        for ch in text.chars() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(1).clamp(1, 2) as u16;
            if cells_width(buf) as u16 + ch_w > width {
                finish_row(buf, width, rows);
            }
            push_char(buf, ch, style, focus);
        }
    };

    match line {
        PageLine::Blank => {
            finish_row(&mut buf, width, rows);
        }
        PageLine::Heading(text, level, ts) => {
            let mut style = *ts;
            style.bold = true;
            if *level == 1 { style.underline = true; }
            push(&mut buf, rows, text, style, None);
            finish_row(&mut buf, width, rows);
        }
        PageLine::Text(segments) => {
            for seg in segments {
                let focus = seg.link_idx.map(FocusKind::Link);
                push(&mut buf, rows, &seg.text, seg.style, focus);
            }
            finish_row(&mut buf, width, rows);
        }
        PageLine::InputRef(placeholder, idx, ts) => {
            let value = match input_override {
                Some((edit_idx, v)) if edit_idx == *idx => v,
                _ => page.inputs.get(*idx).map(|i| i.value.as_str()).unwrap_or(""),
            };
            let display = if !value.is_empty() {
                format!("[{}]", value)
            } else {
                format!("[{}]", placeholder)
            };
            push(&mut buf, rows, &display, *ts, Some(FocusKind::Input(*idx)));
            finish_row(&mut buf, width, rows);
        }
        PageLine::ButtonRef(label, idx, ts) => {
            let display = format!("[{}]", label);
            push(&mut buf, rows, &display, *ts, Some(FocusKind::Button(*idx)));
            finish_row(&mut buf, width, rows);
        }
    }
}
