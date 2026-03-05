use std::sync::OnceLock;

use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color as RatColor, Style},
    widgets::StatefulWidget,
};

use super::Rain;

/// Check once at startup whether the terminal supports 24-bit true color.
fn supports_truecolor() -> bool {
    static RESULT: OnceLock<bool> = OnceLock::new();
    *RESULT.get_or_init(|| {
        matches!(
            std::env::var("COLORTERM").as_deref(),
            Ok("truecolor") | Ok("24bit")
        )
    })
}

/// Convert an RGB triple to the nearest xterm-256 color index.
fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Grayscale shortcut
    if r == g && g == b {
        if r < 8 {
            return 16; // black end of color cube
        }
        if r > 248 {
            return 231; // white end of color cube
        }
        return 232 + ((r as f32 - 8.0) / 247.0 * 23.0).round() as u8;
    }
    // Map into the 6x6x6 color cube (indices 16–231)
    let ri = (r as f32 / 255.0 * 5.0).round() as u8;
    let gi = (g as f32 / 255.0 * 5.0).round() as u8;
    let bi = (b as f32 / 255.0 * 5.0).round() as u8;
    16 + 36 * ri + 6 * gi + bi
}

/// Convert [r, g, b] to a RatColor that works on the current terminal.
fn to_color(rgb: [u8; 3]) -> RatColor {
    let [r, g, b] = rgb;
    if supports_truecolor() {
        RatColor::Rgb(r, g, b)
    } else {
        RatColor::Indexed(rgb_to_ansi256(r, g, b))
    }
}

/// Renders Rain's screen_buffer into a ratatui Buffer.
/// No rain logic here — just reads the buffer and maps cells.
pub struct RainWidget {
    pub bg_color: Option<RatColor>,
}

impl RainWidget {
    pub fn new() -> Self {
        Self { bg_color: None }
    }

    pub fn bg(mut self, color: Option<(u8, u8, u8)>) -> Self {
        self.bg_color = color.map(|(r, g, b)| to_color([r, g, b]));
        self
    }
}

impl StatefulWidget for RainWidget {
    type State = Rain<1024>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Fill background if set
        if let Some(bg) = self.bg_color {
            let bg_style = Style::default().bg(bg);
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                        cell.set_style(bg_style);
                    }
                }
            }
        }

        // Map Rain's logical columns to ratatui buffer cells
        let rain_h = state.height.min(area.height as usize);
        let rain_w = state.width.min(area.width as usize / state.char_width.max(1));

        for y in 0..rain_h {
            for x in 0..rain_w {
                let buf_idx = y * state.width + x;
                if buf_idx >= state.screen_buffer.len() {
                    continue;
                }
                let cell = &state.screen_buffer[buf_idx];
                let screen_x = area.x + (x * state.char_width) as u16;
                let screen_y = area.y + y as u16;

                if screen_x >= area.x + area.width || screen_y >= area.y + area.height {
                    continue;
                }

                let fg = to_color(cell.color);

                if let Some(buf_cell) = buf.cell_mut(Position::new(screen_x, screen_y)) {
                    if cell.is_visible() {
                        buf_cell.set_char(cell.char);
                    } else {
                        buf_cell.set_char(' ');
                    }
                    buf_cell.set_fg(fg);
                    if let Some(bg) = self.bg_color {
                        buf_cell.set_bg(bg);
                    }
                }
            }
        }
    }
}
