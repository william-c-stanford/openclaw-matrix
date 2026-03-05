use crate::config::Charset;
use crate::rain::characters::random_char;

/// A single character dissolving from the input box into rain
pub struct DissolveParticle {
    pub x: u16,
    pub y: f32,
    pub speed: f32,
    pub ch: char,
    pub ticks_alive: u16,
    pub mutate_after: u16, // Ticks before char mutates to katakana
}

impl DissolveParticle {
    pub fn new(x: u16, y: u16, ch: char, rng: &mut fastrand::Rng) -> Self {
        Self {
            x,
            y: y as f32,
            speed: 0.4 + rng.f32() * 0.8, // 0.4-1.2 cells/tick
            ch,
            ticks_alive: 0,
            mutate_after: 3 + rng.u16(..5), // Mutate after 3-7 ticks
        }
    }

    pub fn tick(&mut self, charset: Charset, rng: &mut fastrand::Rng) {
        self.y += self.speed;
        self.ticks_alive += 1;
        if self.ticks_alive >= self.mutate_after {
            self.ch = random_char(rng, charset);
        }
    }

    pub fn screen_y(&self) -> i32 {
        self.y.floor() as i32
    }

    pub fn is_offscreen(&self, height: u16) -> bool {
        self.screen_y() >= height as i32
    }

    /// Brightness factor (1.0 = bright, fading as it falls)
    pub fn brightness(&self) -> f32 {
        let age = self.ticks_alive as f32;
        (1.0 - age / 40.0).max(0.1)
    }
}

/// Dissolve effect: submitted text breaks apart into rain
pub struct DissolveEffect {
    pub particles: Vec<DissolveParticle>,
}

impl DissolveEffect {
    pub fn new(text: &str, input_x: u16, input_y: u16, rng: &mut fastrand::Rng) -> Self {
        let particles = text
            .chars()
            .enumerate()
            .map(|(i, ch)| DissolveParticle::new(input_x + i as u16, input_y, ch, rng))
            .collect();
        Self { particles }
    }

    pub fn tick(&mut self, height: u16, charset: Charset, rng: &mut fastrand::Rng) {
        for p in &mut self.particles {
            p.tick(charset, rng);
        }
        self.particles.retain(|p| !p.is_offscreen(height));
    }

    pub fn is_done(&self) -> bool {
        self.particles.is_empty()
    }
}
