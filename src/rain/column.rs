use crate::config::Charset;
use crate::rain::characters::random_char;

pub struct RainColumn {
    pub x: u16,
    pub head_y: f32,
    pub speed: f32,
    pub trail_length: u16,
    pub chars: Vec<char>,
    pub active: bool,
    pub respawn_delay: u16,
    last_int_y: i32,
}

impl RainColumn {
    pub fn new_dormant(x: u16, height: u16, rng: &mut fastrand::Rng) -> Self {
        Self {
            x,
            head_y: 0.0,
            speed: 0.0,
            trail_length: 10,
            chars: Vec::new(),
            active: false,
            respawn_delay: rng.u16(..height.max(1)),
            last_int_y: -1,
        }
    }

    pub fn new_active(x: u16, height: u16, base_speed: f32, charset: Charset, rng: &mut fastrand::Rng) -> Self {
        let speed = base_speed * (0.5 + rng.f32());
        let trail_length = 5 + rng.u16(..26);
        let head_y = rng.f32() * (height as f32 + trail_length as f32);
        let last_int_y = head_y.floor() as i32;
        let num_chars = trail_length as usize;
        let chars: Vec<char> = (0..num_chars).map(|_| random_char(rng, charset)).collect();

        Self {
            x,
            head_y,
            speed,
            trail_length,
            chars,
            active: true,
            respawn_delay: 0,
            last_int_y,
        }
    }

    pub fn activate(&mut self, base_speed: f32, rng: &mut fastrand::Rng) {
        self.speed = base_speed * (0.5 + rng.f32());
        self.trail_length = 5 + rng.u16(..26);
        self.head_y = 0.0;
        self.last_int_y = -1;
        self.chars.clear();
        self.active = true;
        self.respawn_delay = 0;
    }

    pub fn tick(&mut self, height: u16, base_speed: f32, charset: Charset, rng: &mut fastrand::Rng) {
        if !self.active {
            if self.respawn_delay == 0 {
                self.activate(base_speed, rng);
            } else {
                self.respawn_delay -= 1;
            }
            return;
        }

        self.head_y += self.speed;

        let current_int_y = self.head_y.floor() as i32;
        if current_int_y > self.last_int_y {
            let steps = ((current_int_y - self.last_int_y) as usize).min(5);
            for _ in 0..steps {
                self.chars.insert(0, random_char(rng, charset));
            }
            self.chars.truncate(self.trail_length as usize);
            self.last_int_y = current_int_y;
        }

        // Mutate ~2% of existing characters (5/256 = 1.95%)
        for ch in &mut self.chars {
            if rng.u8(..) < 5 {
                *ch = random_char(rng, charset);
            }
        }

        // Deactivate when entire trail has scrolled past bottom
        if (self.head_y as i32) - (self.trail_length as i32) >= height as i32 {
            self.active = false;
            self.respawn_delay = rng.u16(..height.max(1));
        }
    }

    /// Returns iterator of (y_position, distance_from_head, char) for visible cells
    pub fn visible_cells(&self, height: u16) -> impl Iterator<Item = (u16, usize, char)> + '_ {
        let head_int = self.head_y.floor() as i32;
        self.chars.iter().enumerate().filter_map(move |(i, &ch)| {
            let y = head_int - i as i32;
            if y >= 0 && y < height as i32 {
                Some((y as u16, i, ch))
            } else {
                None
            }
        })
    }
}
