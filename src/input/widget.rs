use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::InputState;

/// Renders the input box at the bottom-left of the screen
pub struct InputWidget<'a> {
    state: &'a InputState,
    focused: bool,
}

impl<'a> InputWidget<'a> {
    pub fn new(state: &'a InputState, focused: bool) -> Self {
        Self { state, focused }
    }

    /// Input area: left-aligned, matching chat width, 3 rows tall at bottom
    pub fn input_area(area: Rect) -> Rect {
        let width = (area.width as f32 * 0.45).max(30.0).min(area.width as f32) as u16;
        let height = 3;
        let y = area.y + area.height.saturating_sub(height + 1); // +1 for status bar
        Rect::new(area.x + 1, y, width, height)
    }
}

impl Widget for InputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.focused {
            Color::Green
        } else {
            Color::DarkGray
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(if self.focused { " type... " } else { " i:chat " });

        let inner = block.inner(area);
        block.render(area, buf);

        // Render text content
        let display_text = if self.state.is_empty() && !self.focused {
            String::new()
        } else {
            self.state.text.clone()
        };

        // Simple single-line render (scroll if text exceeds width)
        let inner_width = inner.width as usize;
        let cursor_pos = self.state.cursor;
        let text_chars: Vec<char> = display_text.chars().collect();

        // Calculate visible window
        let scroll = if cursor_pos >= inner_width {
            cursor_pos - inner_width + 1
        } else {
            0
        };

        let visible: String = text_chars
            .iter()
            .skip(scroll)
            .take(inner_width)
            .collect();

        let text_widget = Paragraph::new(visible)
            .style(Style::default().fg(Color::White));
        text_widget.render(inner, buf);

        // Set cursor position for blinking cursor
        if self.focused && inner.width > 0 && inner.height > 0 {
            let cursor_x = inner.x + (cursor_pos - scroll).min(inner_width.saturating_sub(1)) as u16;
            if let Some(cell) = buf.cell_mut(Position::new(cursor_x, inner.y)) {
                cell.set_style(Style::default().fg(Color::Black).bg(Color::Green));
            }
        }
    }
}
