use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Widget},
};

use super::SettingsState;

/// Full-screen settings widget that replaces the rain
pub struct SettingsWidget<'a> {
    state: &'a SettingsState,
}

impl<'a> SettingsWidget<'a> {
    pub fn new(state: &'a SettingsState) -> Self {
        Self { state }
    }
}

impl Widget for SettingsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the entire screen with a dark background
        Clear.render(area, buf);
        let bg_style = Style::default().bg(Color::Black);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                    cell.set_style(bg_style);
                }
            }
        }

        // Draw border
        let block = Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 20 {
            return;
        }

        // Header
        let header = "  Arrow keys: navigate  Left/Right: change  Esc: apply & close";
        render_str(buf, inner.x + 1, inner.y, header, Style::default().fg(Color::DarkGray));

        // Settings entries
        let start_y = inner.y + 2;
        for i in 0..self.state.entry_count() {
            let y = start_y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = i == self.state.cursor;
            let label = self.state.entry_label(i);
            let value = self.state.entry_value(i);

            let label_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let value_style = if is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Render: "  Label      < value >"
            let prefix = if is_selected { "> " } else { "  " };
            let line = format!("{prefix}{label:<12}  < {value} >");
            render_str(buf, inner.x + 1, y, &line[..prefix.len() + label.len() + 2], label_style);

            // Render the value part separately for different coloring
            let value_part = format!("< {value} >");
            let value_x = inner.x + 1 + prefix.len() as u16 + 12 + 2;
            render_str(buf, value_x, y, &value_part, value_style);
        }
    }
}

fn render_str(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style) {
    let mut cx = x;
    for ch in text.chars() {
        if let Some(cell) = buf.cell_mut(Position::new(cx, y)) {
            cell.set_char(ch);
            cell.set_style(style);
        }
        cx += 1;
    }
}
