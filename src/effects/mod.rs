use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Color,
    widgets::Widget,
};
use std::time::Instant;

/// Visual effect triggered on message send
#[derive(Debug)]
pub struct Effect {
    kind: EffectKind,
    start: Instant,
    duration_ms: u64,
    center_x: u16,
    center_y: u16,
}

#[derive(Debug, Clone, Copy)]
enum EffectKind {
    Burst,
    Dissolve,
    Glitch,
}

impl Effect {
    pub fn burst(x: u16, y: u16) -> Self {
        Self {
            kind: EffectKind::Burst,
            start: Instant::now(),
            duration_ms: 400,
            center_x: x,
            center_y: y,
        }
    }

    pub fn dissolve(x: u16, y: u16) -> Self {
        Self {
            kind: EffectKind::Dissolve,
            start: Instant::now(),
            duration_ms: 600,
            center_x: x,
            center_y: y,
        }
    }

    pub fn glitch(x: u16, y: u16) -> Self {
        Self {
            kind: EffectKind::Glitch,
            start: Instant::now(),
            duration_ms: 300,
            center_x: x,
            center_y: y,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.start.elapsed().as_millis() as u64 > self.duration_ms
    }

    fn progress(&self) -> f32 {
        let elapsed = self.start.elapsed().as_millis() as f32;
        (elapsed / self.duration_ms as f32).min(1.0)
    }
}

/// Manages active visual effects
pub struct EffectManager {
    effects: Vec<Effect>,
    rng_state: u32,
}

impl EffectManager {
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            rng_state: 42,
        }
    }

    pub fn trigger(&mut self, x: u16, y: u16) {
        // Cycle through effect types
        let kind_idx = self.effects.len() % 3;
        let effect = match kind_idx {
            0 => Effect::burst(x, y),
            1 => Effect::dissolve(x, y),
            _ => Effect::glitch(x, y),
        };
        self.effects.push(effect);
    }

    pub fn tick(&mut self) {
        self.effects.retain(|e| !e.is_expired());
    }

    pub fn has_active(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Simple pseudo-random for effect rendering
    #[allow(dead_code)]
    fn next_rand(&mut self) -> u32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        self.rng_state
    }
}

/// Widget that renders all active effects
pub struct EffectsWidget<'a> {
    manager: &'a mut EffectManager,
}

impl<'a> EffectsWidget<'a> {
    pub fn new(manager: &'a mut EffectManager) -> Self {
        Self { manager }
    }
}

impl Widget for EffectsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for effect in &self.manager.effects {
            let progress = effect.progress();
            match effect.kind {
                EffectKind::Burst => render_burst(effect, progress, area, buf),
                EffectKind::Dissolve => render_dissolve(effect, progress, area, buf, self.manager.rng_state),
                EffectKind::Glitch => render_glitch(effect, progress, area, buf, self.manager.rng_state),
            }
        }
        // Advance RNG state
        self.manager.rng_state ^= self.manager.rng_state << 13;
        self.manager.rng_state ^= self.manager.rng_state >> 17;
        self.manager.rng_state ^= self.manager.rng_state << 5;
    }
}

fn render_burst(effect: &Effect, progress: f32, area: Rect, buf: &mut Buffer) {
    let radius = (progress * 10.0) as i16;
    let brightness = (255.0 * (1.0 - progress)) as u8;
    let color = Color::Rgb(brightness, brightness.saturating_add(50), brightness);

    let chars = ['*', '+', '.', '`'];
    for angle_step in 0..8 {
        let angle = (angle_step as f32) * std::f32::consts::PI / 4.0;
        let dx = (angle.cos() * radius as f32) as i16;
        let dy = (angle.sin() * radius as f32 / 2.0) as i16; // half height for terminal chars

        let x = effect.center_x as i16 + dx;
        let y = effect.center_y as i16 + dy;

        if x >= area.x as i16
            && x < (area.x + area.width) as i16
            && y >= area.y as i16
            && y < (area.y + area.height) as i16
        {
            if let Some(cell) = buf.cell_mut(Position::new(x as u16, y as u16)) {
                cell.set_char(chars[(angle_step + radius as usize) % chars.len()]);
                cell.set_fg(color);
            }
        }
    }
}

fn render_dissolve(effect: &Effect, progress: f32, area: Rect, buf: &mut Buffer, rng: u32) {
    let spread = (progress * 15.0) as u16;
    let alpha = (255.0 * (1.0 - progress)) as u8;

    for i in 0..12 {
        let hash = rng.wrapping_mul(i + 1).wrapping_add(effect.start.elapsed().as_millis() as u32);
        let dx = (hash % (spread as u32 * 2 + 1)) as i16 - spread as i16;
        let dy = ((hash >> 8) % (spread as u32 + 1)) as i16 - (spread as i16 / 2);

        let x = effect.center_x as i16 + dx;
        let y = effect.center_y as i16 + dy;

        if x >= area.x as i16
            && x < (area.x + area.width) as i16
            && y >= area.y as i16
            && y < (area.y + area.height) as i16
        {
            if let Some(cell) = buf.cell_mut(Position::new(x as u16, y as u16)) {
                cell.set_char(if progress < 0.5 { '#' } else { '.' });
                cell.set_fg(Color::Rgb(0, alpha, 0));
            }
        }
    }
}

fn render_glitch(effect: &Effect, progress: f32, area: Rect, buf: &mut Buffer, rng: u32) {
    let intensity = 1.0 - progress;
    let num_lines = (intensity * 5.0) as u16;

    for i in 0..num_lines {
        let hash = rng.wrapping_mul(i as u32 + 1).wrapping_add(effect.start.elapsed().as_millis() as u32);
        let y_offset = (hash % area.height as u32) as u16;
        let y = area.y + y_offset;
        let x_shift = ((hash >> 16) % 6) as i16 - 3;

        let glitch_width = ((hash >> 8) % 10 + 3) as u16;
        let start_x = effect.center_x.saturating_sub(glitch_width / 2);

        for x in start_x..start_x.saturating_add(glitch_width) {
            let shifted_x = (x as i16 + x_shift).max(area.x as i16) as u16;
            if shifted_x < area.x + area.width && y < area.y + area.height {
                if let Some(cell) = buf.cell_mut(Position::new(shifted_x, y)) {
                    let glitch_char = ['|', '/', '-', '\\', '_'][(hash as usize + x as usize) % 5];
                    cell.set_char(glitch_char);
                    cell.set_fg(Color::Rgb(0, (intensity * 255.0) as u8, 0));
                }
            }
        }
    }
}
