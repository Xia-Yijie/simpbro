// Mouse interaction: click to focus, drag to select, OSC 52 to clipboard.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::browser::{FocusItem, FocusKind, Page};
use crate::viewport::Viewport;

pub enum MouseAction {
    None,
    ScrollUp(usize),
    ScrollDown(usize),
    /// Click landed on an interactive cell — move focus to it.
    FocusAt(usize),
    /// Double-click on an interactive cell — focus then activate.
    ActivateAt(usize),
    /// Selection released with text → send to clipboard.
    Copy(String),
    /// Click on a tab label → switch to that tab.
    SwitchTab(usize),
}

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct MouseState {
    /// Drag start position in content-area-local (col, row) coords.
    drag_start: Option<(u16, u16)>,
    /// Current drag end.
    drag_end: Option<(u16, u16)>,
    /// Track previous click for double-click detection.
    last_click: Option<(Instant, u16, u16)>,
}

impl MouseState {
    pub fn selection(&self) -> Option<((u16, u16), (u16, u16))> {
        match (self.drag_start, self.drag_end) {
            (Some(a), Some(b)) if a != b => Some((a, b)),
            _ => None,
        }
    }

    pub fn clear_selection(&mut self) {
        self.drag_start = None;
        self.drag_end = None;
    }

    pub fn handle(
        &mut self,
        ev: &MouseEvent,
        content_area: Rect,
        tab_bar_area: Rect,
        tab_regions: &[(u16, u16)],
        viewport: &Viewport,
        page: Option<&Page>,
    ) -> MouseAction {
        match ev.kind {
            MouseEventKind::ScrollUp => return MouseAction::ScrollUp(3),
            MouseEventKind::ScrollDown => return MouseAction::ScrollDown(3),
            _ => {}
        }

        // Tab bar: left-click on a tab label switches tabs.
        if matches!(ev.kind, MouseEventKind::Down(MouseButton::Left))
            && in_rect(tab_bar_area, ev.column, ev.row)
        {
            for (i, (lo, hi)) in tab_regions.iter().enumerate() {
                if ev.column >= *lo && ev.column <= *hi {
                    return MouseAction::SwitchTab(i);
                }
            }
            return MouseAction::None;
        }

        if !in_rect(content_area, ev.column, ev.row) {
            return MouseAction::None;
        }
        let col = ev.column - content_area.x;
        let row = ev.row - content_area.y;

        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Reset any prior selection
                self.clear_selection();
                self.drag_start = Some((col, row));
                self.drag_end = None;

                // Double-click detection
                let now = Instant::now();
                let is_double = matches!(
                    self.last_click,
                    Some((t, c, r)) if now.duration_since(t) < DOUBLE_CLICK_WINDOW && c == col && r == row
                );
                self.last_click = Some((now, col, row));

                let hit = viewport.focus_at(col, row);
                if let Some(kind) = hit {
                    let idx = page_focus_index_for(page, kind);
                    if let Some(idx) = idx {
                        return if is_double {
                            MouseAction::ActivateAt(idx)
                        } else {
                            MouseAction::FocusAt(idx)
                        };
                    }
                }
                MouseAction::None
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.drag_end = Some((col, row));
                MouseAction::None
            }
            // Right-click copies the active selection to the clipboard; left
            // release keeps the selection visible (falls through to default).
            MouseEventKind::Down(MouseButton::Right) => {
                if let (Some(a), Some(b)) = (self.drag_start, self.drag_end) {
                    if a != b {
                        let text = viewport.extract_text(a, b);
                        if !text.is_empty() {
                            return MouseAction::Copy(text);
                        }
                    }
                }
                MouseAction::None
            }
            _ => MouseAction::None,
        }
    }
}

pub fn in_rect(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

fn page_focus_index_for(page: Option<&Page>, kind: FocusKind) -> Option<usize> {
    let page = page?;
    page.focus_order.iter().position(|fi: &FocusItem| fi.kind == kind)
}

/// Send text to the system clipboard via OSC 52. Works through SSH.
pub fn osc52_copy(text: &str) -> io::Result<()> {
    let b64 = base64_encode(text.as_bytes());
    let mut out = io::stdout();
    // \x1b]52;c;<base64>\x07
    write!(out, "\x1b]52;c;{}\x07", b64)?;
    out.flush()
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let n = (u32::from(input[i]) << 16) | (u32::from(input[i + 1]) << 8) | u32::from(input[i + 2]);
        out.push(CHARS[(n >> 18 & 0x3f) as usize] as char);
        out.push(CHARS[(n >> 12 & 0x3f) as usize] as char);
        out.push(CHARS[(n >> 6 & 0x3f) as usize] as char);
        out.push(CHARS[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = u32::from(input[i]) << 16;
        out.push(CHARS[(n >> 18 & 0x3f) as usize] as char);
        out.push(CHARS[(n >> 12 & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = (u32::from(input[i]) << 16) | (u32::from(input[i + 1]) << 8);
        out.push(CHARS[(n >> 18 & 0x3f) as usize] as char);
        out.push(CHARS[(n >> 12 & 0x3f) as usize] as char);
        out.push(CHARS[(n >> 6 & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}
