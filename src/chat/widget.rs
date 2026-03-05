use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Widget,
};

use super::{ChatState, Role};

/// Renders chat messages as an overlay on the left side of the screen.
/// Messages grow bottom-up: most recent message is always at the bottom.
pub struct ChatWidget<'a> {
    state: &'a ChatState,
}

impl<'a> ChatWidget<'a> {
    pub fn new(state: &'a ChatState) -> Self {
        Self { state }
    }

    /// Chat area: left-aligned, ~45% width, full height minus input and status bar
    pub fn chat_area(area: Rect) -> Rect {
        let chat_width = (area.width as f32 * 0.45).max(30.0).min(area.width as f32) as u16;
        // Leave 4 rows at bottom for input box (3) + status bar (1)
        let chat_height = area.height.saturating_sub(4);
        Rect::new(area.x + 1, area.y, chat_width, chat_height)
    }
}

impl Widget for ChatWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 10 {
            return;
        }

        // Build all rendered lines from messages
        let mut all_lines: Vec<Line<'_>> = Vec::new();

        for msg in &self.state.messages {
            let (prefix, style) = match msg.role {
                Role::User => (
                    "you: ",
                    Style::default().fg(Color::White),
                ),
                Role::Assistant => (
                    "claw: ",
                    Style::default().fg(Color::Green),
                ),
                Role::System => (
                    "sys: ",
                    Style::default().fg(Color::DarkGray),
                ),
            };

            let wrapped = word_wrap(&format!("{prefix}{}", msg.content), area.width as usize);
            for line_str in wrapped {
                all_lines.push(Line::from(Span::styled(line_str, style)));
            }
            all_lines.push(Line::from("")); // blank line between messages
        }

        // Add streaming content if present
        if let Some(ref streaming) = self.state.streaming {
            let style = Style::default().fg(Color::Green);
            let wrapped = word_wrap(&format!("claw: {streaming}"), area.width as usize);
            for line_str in wrapped {
                all_lines.push(Line::from(Span::styled(line_str, style)));
            }
            all_lines.push(Line::from(Span::styled("...", Style::default().fg(Color::DarkGray))));
        }

        // Render bottom-up: most recent message pinned to bottom of area
        let visible_height = area.height as usize;
        let total_lines = all_lines.len();
        let skip = self.state.scroll_offset.min(total_lines.saturating_sub(visible_height));
        let end = total_lines.saturating_sub(skip);
        let start = end.saturating_sub(visible_height);
        let visible_count = end - start;

        // Pin to bottom: first visible line starts at (area.bottom - visible_count)
        let y_offset = area.y + area.height.saturating_sub(visible_count as u16);

        for (i, line) in all_lines[start..end].iter().enumerate() {
            let y = y_offset + i as u16;
            if y >= area.y + area.height {
                break;
            }
            let mut x = area.x;
            for span in &line.spans {
                for ch in span.content.chars() {
                    if x >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                        cell.set_char(ch);
                        cell.set_style(span.style);
                    }
                    x += 1;
                }
            }
        }
    }
}

/// Simple word wrapping
fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![];
    }
    let mut lines = Vec::new();
    for input_line in text.split('\n') {
        if input_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in input_line.split_whitespace() {
            if current.is_empty() {
                if word.len() > max_width {
                    for chunk in word.as_bytes().chunks(max_width) {
                        lines.push(String::from_utf8_lossy(chunk).into_owned());
                    }
                } else {
                    current = word.to_string();
                }
            } else if current.len() + 1 + word.len() > max_width {
                lines.push(current);
                current = word.to_string();
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
