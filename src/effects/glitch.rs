use crate::config::Charset;
use crate::rain::characters::random_char;

/// Glitch effect: horizontal character displacement near the input area
pub struct GlitchEffect {
    pub frames_remaining: u8,
    pub rows: Vec<GlitchRow>,
}

pub struct GlitchRow {
    pub y: u16,
    pub offset: i16, // Horizontal displacement (-3 to +3)
    pub chars: Vec<char>, // Random chars to overlay
    pub _width: u16,
}

impl GlitchEffect {
    pub fn new(input_y: u16, screen_height: u16, _screen_width: u16, charset: Charset, rng: &mut fastrand::Rng) -> Self {
        let num_rows = 3 + rng.usize(..4); // 3-6 glitch rows
        let rows = (0..num_rows)
            .map(|_| {
                let y_offset = rng.i16(-5..6);
                let y = (input_y as i16 + y_offset).clamp(0, screen_height as i16 - 1) as u16;
                let width = 5 + rng.u16(..15); // 5-19 chars wide
                let offset = rng.i16(-3..4);
                let chars = (0..width).map(|_| random_char(rng, charset)).collect();
                GlitchRow { y, offset, chars, _width: width }
            })
            .collect();

        Self {
            frames_remaining: 2 + rng.u8(..2), // 2-3 frames
            rows,
        }
    }

    pub fn tick(&mut self) {
        self.frames_remaining = self.frames_remaining.saturating_sub(1);
    }

    pub fn is_done(&self) -> bool {
        self.frames_remaining == 0
    }
}
