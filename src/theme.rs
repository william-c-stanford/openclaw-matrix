use ratatui::style::Color;

pub struct Theme {
    pub head: Color,
    pub base_r: u8,
    pub base_g: u8,
    pub base_b: u8,
}

impl Theme {
    pub fn from_color_str(s: &str) -> Self {
        let (r, g, b) = parse_color(s);
        let head = Color::Rgb(
            r.saturating_add(150),
            g.saturating_add(150),
            b.saturating_add(150),
        );
        Self { head, base_r: r, base_g: g, base_b: b }
    }

    pub fn trail_color(&self, distance_from_head: usize, trail_length: usize) -> Color {
        if distance_from_head == 0 {
            self.head
        } else {
            let ratio = distance_from_head as f32 / trail_length.max(1) as f32;
            let factor = (1.0 - ratio * 0.85).max(0.0);
            Color::Rgb(
                (self.base_r as f32 * factor) as u8,
                (self.base_g as f32 * factor) as u8,
                (self.base_b as f32 * factor) as u8,
            )
        }
    }
}

fn parse_color(s: &str) -> (u8, u8, u8) {
    match s.to_lowercase().as_str() {
        "green" => (0, 255, 0),
        "blue" => (0, 100, 255),
        "red" => (255, 0, 50),
        "cyan" => (0, 255, 255),
        "purple" => (180, 0, 255),
        "white" => (255, 255, 255),
        "yellow" => (255, 255, 0),
        hex if hex.starts_with('#') && hex.len() == 7 => {
            let r = u8::from_str_radix(&hex[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[3..5], 16).unwrap_or(255);
            let b = u8::from_str_radix(&hex[5..7], 16).unwrap_or(0);
            (r, g, b)
        }
        _ => (0, 255, 0),
    }
}
