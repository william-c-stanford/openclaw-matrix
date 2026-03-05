/// Rain burst: temporarily increase speed and brightness near the input area
pub struct BurstEffect {
    pub center_x: u16,
    pub radius: u16,
    pub ticks_remaining: u16,
    pub total_ticks: u16,
}

impl BurstEffect {
    pub fn new(center_x: u16) -> Self {
        let total = 30; // ~500ms at 60 FPS
        Self {
            center_x,
            radius: 15,
            ticks_remaining: total,
            total_ticks: total,
        }
    }

    pub fn tick(&mut self) {
        self.ticks_remaining = self.ticks_remaining.saturating_sub(1);
    }

    pub fn is_done(&self) -> bool {
        self.ticks_remaining == 0
    }

    #[allow(dead_code)]
    pub fn speed_multiplier(&self, col_x: u16) -> f32 {
        let dist = (col_x as i32 - self.center_x as i32).unsigned_abs() as u16;
        if dist > self.radius {
            return 1.0;
        }
        let progress = self.ticks_remaining as f32 / self.total_ticks as f32;
        let proximity = 1.0 - (dist as f32 / self.radius as f32);
        1.0 + proximity * progress * 1.5 // Up to 2.5x speed at center
    }

    /// Brightness boost (0.0 = none, 1.0 = max)
    pub fn brightness_boost(&self, col_x: u16) -> f32 {
        let dist = (col_x as i32 - self.center_x as i32).unsigned_abs() as u16;
        if dist > self.radius {
            return 0.0;
        }
        let progress = self.ticks_remaining as f32 / self.total_ticks as f32;
        let proximity = 1.0 - (dist as f32 / self.radius as f32);
        proximity * progress * 0.5
    }
}
