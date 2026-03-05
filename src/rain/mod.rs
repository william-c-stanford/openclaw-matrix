pub mod widget;

use rand::RngExt;
use std::time::{Duration, Instant};

#[cfg(test)]
use rand::SeedableRng;

use crate::cli;

pub const MAXSPEED: u64 = 0;
pub const MINSPEED: u64 = 200;

/// rand crate wrapper for testing.
/// being able to have deterministic tests is important
#[derive(Debug)]
pub struct Random {
    #[cfg(test)]
    rng: rand::rngs::StdRng,
    #[cfg(not(test))]
    rng: rand::rngs::ThreadRng,
}

impl Default for Random {
    fn default() -> Self {
        Self {
            #[cfg(test)]
            rng: rand::rngs::StdRng::seed_from_u64(42),
            #[cfg(not(test))]
            rng: rand::rng(),
        }
    }
}

impl Random {
    pub fn random_range<T, R>(&mut self, range: R) -> T
    where
        T: rand::distr::uniform::SampleUniform + PartialOrd,
        R: rand::distr::uniform::SampleRange<T>,
    {
        self.rng.random_range(range)
    }

    /// Returns a random float in [0.0, 1.0)
    pub fn random_float(&mut self) -> f32 {
        self.rng.random_range(0.0f32..1.0f32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cell {
    pub(crate) char: char,
    pub(crate) color: [u8; 3],
}

impl Cell {
    fn new(char: char) -> Self {
        Self {
            char,
            color: [0, 0, 0],
        }
    }

    fn color(mut self, color: [u8; 3]) -> Self {
        self.color = color;
        self
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.char != ' '
    }

    #[cfg(test)]
    pub(crate) fn display(&self, width: usize) -> String {
        if width >= 2 && !self.is_visible() {
            " ".repeat(width)
        } else {
            self.char.to_string()
        }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            char: ' ',
            color: [0, 0, 0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl std::str::FromStr for Direction {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "north" => Ok(Self::Up),
            "south" => Ok(Self::Down),
            "west" => Ok(Self::Left),
            "east" => Ok(Self::Right),
            _ => Err(format!("Invalid direction: {value}")),
        }
    }
}

#[derive(Debug)]
pub struct Rain<const LENGTH: usize> {
    /// Random number generator wrapper for testing purposes
    pub(crate) rng: Random,
    /// Characters to use for the rain
    chars: [char; LENGTH],
    /// Starting positions of the rain within the chars array
    starts: Vec<usize>,
    /// Window size for each column of rain
    windows: Vec<usize>,
    /// Current positions of the rain falling
    positions: Vec<usize>,
    /// Color of the rain body
    body_colors: Vec<([u8; 3], Option<Vec<[u8; 3]>>)>,
    /// Shading of the rain
    shading: bool,
    /// Color to fade into when shading is enabled
    shade_gradient: [u8; 3],
    /// Color of the rain head
    head_colors: Vec<[u8; 3]>,
    /// Direction of the rain
    directions: Vec<Direction>,
    /// Animation timing
    time: Vec<(Instant, Duration)>,
    /// List of columns that need to be updated
    queue: Vec<usize>,
    /// Speed of the rain
    speed: std::ops::Range<u64>,
    /// Character width
    pub(crate) char_width: usize,
    /// Width of the terminal (in logical columns, not terminal columns)
    pub(crate) width: usize,
    /// Height of the terminal
    pub(crate) height: usize,
    /// Current screen buffer
    pub(crate) screen_buffer: Vec<Cell>,
    /// Previous screen buffer (retained for parity with rusty-rain; used by tests)
    #[allow(dead_code)]
    previous_screen_buffer: Vec<Cell>,

    // --- Mood override fields ---
    /// Override body color for new drops (set by MoodDirector)
    override_body_color: Option<[u8; 3]>,
    /// Override head color for new drops (set by MoodDirector)
    override_head_color: Option<[u8; 3]>,
    /// Per-column emoji head override (Some(emoji) = use emoji as head char)
    emoji_heads: Vec<Option<char>>,
    /// Emoji pool for accent sampling (set by MoodDirector)
    emoji_pool: Vec<char>,
    /// Fraction of strands that get emoji head on reset (0.0-1.0)
    emoji_density: f32,
}

impl<const LENGTH: usize> Rain<LENGTH> {
    const MIN_LENGTH_OF_RAIN: usize = 4;
    const MAX_LENGTH_OFFSET_OF_RAIN: usize = 4;
    pub fn new(mut width: usize, height: usize, settings: &cli::Cli) -> Self {
        width /= settings.group.width() as usize;

        let mut rng = Random::default();
        let char_length = settings.group.len();
        let chars: [char; LENGTH] = std::array::from_fn(|_| {
            settings
                .group
                .nth_char(rng.random_range(0..char_length))
                .unwrap_or('#') // fallback character
        });

        let starts: Vec<usize> = (0..width)
            .map(|_| rng.random_range(0..chars.len()))
            .collect();

        let window_height = match settings.direction {
            Direction::Up | Direction::Down => height,
            Direction::Left | Direction::Right => width,
        };

        let windows: Vec<usize> = (0..width)
            .map(|_| {
                rng.random_range(
                    Self::MIN_LENGTH_OF_RAIN
                        ..window_height.saturating_sub(Self::MAX_LENGTH_OFFSET_OF_RAIN),
                )
            })
            .collect();

        let speed = settings.speed_range();
        let now = Instant::now();
        let time: Vec<(Instant, Duration)> = (0..width)
            .map(|_| {
                let milli_seconds = rng.random_range(speed.start..speed.end);
                let duration = Duration::from_millis(milli_seconds);
                let future_delay_ms = rng.random_range(0..2000);
                let start = now + Duration::from_millis(future_delay_ms);

                (start, duration)
            })
            .collect();

        let (br, bg, bb) = settings.rain_color();
        let base_color: [u8; 3] = [br, bg, bb];
        // Shade fades toward the background color (black if none set)
        let shade_color: [u8; 3] = settings
            .rain_bg_color()
            .map(|(r, g, b)| [r, g, b])
            .unwrap_or([0, 0, 0]);
        let body_colors = if settings.shade {
            (0..width)
                .map(|i| {
                    let window = windows[i].saturating_sub(1);
                    let colors = gen_shade_color(base_color, shade_color, window as u8);
                    (base_color, Some(colors))
                })
                .collect::<Vec<_>>()
        } else {
            vec![(base_color, None); width]
        };

        let (hr, hg, hb) = settings.head_color();
        let head_color: [u8; 3] = [hr, hg, hb];

        Self {
            shading: settings.shade,
            shade_gradient: shade_color,
            body_colors,
            chars,
            directions: vec![settings.direction; width],
            char_width: settings.group.width() as usize,
            head_colors: vec![head_color; width],
            height,
            positions: vec![0; width],
            previous_screen_buffer: vec![Cell::default(); width * height],
            queue: Vec::with_capacity(width),
            rng,
            screen_buffer: vec![Cell::default(); width * height],
            speed,
            starts,
            time,
            width,
            windows,
            override_body_color: None,
            override_head_color: None,
            emoji_heads: vec![None; width],
            emoji_pool: Vec::new(),
            emoji_density: 0.0,
        }
    }

    #[inline(always)]
    pub fn update(&mut self) {
        for i in 0..self.width {
            let (start, duration) = self.time[i];
            if start.elapsed() > duration {
                self.queue.push(i);
                let (start, _) = &mut self.time[i];
                *start = Instant::now();
            }
        }
    }

    #[inline(always)]
    fn reset_time(&mut self, i: usize) {
        let (start, duration) = &mut self.time[i];
        *start = Instant::now();
        let milli_seconds = self.rng.random_range(self.speed.start..self.speed.end);
        *duration = Duration::from_millis(milli_seconds);
    }

    #[inline(always)]
    fn reset_start(&mut self, i: usize) {
        self.starts[i] = self.rng.random_range(0..self.chars.len());
    }

    #[inline(always)]
    fn reset_window(&mut self, i: usize) {
        self.windows[i] = self.rng.random_range(
            Self::MIN_LENGTH_OF_RAIN..self.height.saturating_sub(Self::MAX_LENGTH_OFFSET_OF_RAIN),
        );
    }

    #[inline(always)]
    fn reset_position(&mut self, i: usize) {
        self.positions[i] = 0;
    }

    #[inline(always)]
    fn reset_body_colors(&mut self, i: usize) {
        // Use mood override color if set, otherwise keep existing base
        if let Some(override_color) = self.override_body_color {
            self.body_colors[i].0 = override_color;
        }
        if let Some(override_head) = self.override_head_color {
            self.head_colors[i] = override_head;
        }

        if !self.shading {
            return;
        }
        let base_color = self.body_colors[i].0;
        let window = self.windows[i].saturating_sub(1);
        let colors = gen_shade_color(base_color, self.shade_gradient, window as u8);
        self.body_colors[i] = (base_color, Some(colors));
    }

    fn reset(&mut self, i: usize) {
        self.reset_time(i);
        self.reset_start(i);
        self.reset_window(i);
        self.reset_position(i);
        self.reset_body_colors(i);

        // Roll for emoji head accent on this strand
        if !self.emoji_pool.is_empty() && self.emoji_density > 0.0 {
            if self.rng.random_float() < self.emoji_density {
                let idx = self.rng.random_range(0..self.emoji_pool.len());
                self.emoji_heads[i] = Some(self.emoji_pool[idx]);
            } else {
                self.emoji_heads[i] = None;
            }
        } else {
            self.emoji_heads[i] = None;
        }
    }

    /// Set override body/head colors for new drops (mood system)
    pub fn set_override_colors(&mut self, body: Option<[u8; 3]>, head: Option<[u8; 3]>) {
        self.override_body_color = body;
        self.override_head_color = head;
    }

    /// Set emoji accent pool and density (mood system).
    /// Emojis are assigned per-column on reset, not every tick.
    pub fn set_emoji_accents(&mut self, pool: Vec<char>, density: f32) {
        self.emoji_pool = pool;
        self.emoji_density = density.clamp(0.0, 0.25);
    }

    /// Clear all emoji heads and pool
    pub fn clear_emoji_accents(&mut self) {
        self.emoji_pool.clear();
        self.emoji_density = 0.0;
        for head in &mut self.emoji_heads {
            *head = None;
        }
    }

    pub fn update_screen_buffer(&mut self) -> std::io::Result<()> {
        for i in self.queue.drain(..).collect::<Vec<usize>>() {
            let pos = self.positions[i];
            let start_idx = self.starts[i];
            let window_len = self.windows[i];
            let direction = self.directions[i];

            let get_index = |x: usize, y: usize| -> Option<usize> {
                if x < self.width && y < self.height {
                    Some(y * self.width + x)
                } else {
                    None
                }
            };

            let finished = match direction {
                Direction::Down | Direction::Up => pos > (self.height + window_len),
                Direction::Right | Direction::Left => pos > (self.width + window_len),
            };
            if finished {
                self.reset(i);
                continue;
            }

            if pos >= window_len {
                let buf_idx = match direction {
                    Direction::Down => get_index(i, pos - window_len),
                    Direction::Up => {
                        let tail_y = self.height.saturating_sub(pos - window_len + 1);
                        get_index(i, tail_y)
                    }
                    Direction::Right => get_index(pos - window_len, i),
                    Direction::Left => {
                        let tail_x = self.width.saturating_sub(pos - window_len + 1);
                        get_index(tail_x, i)
                    }
                };
                if let Some(idx) = buf_idx {
                    self.screen_buffer[idx] = Cell::default();
                }
            }

            let visible_len = (pos + 1).min(window_len);
            for offset in 0..visible_len {
                let (x, y) = match direction {
                    Direction::Down => (i, pos.saturating_sub(offset)),
                    Direction::Up => (i, self.height.saturating_sub(pos - offset + 1)),
                    Direction::Right => (pos.saturating_sub(offset), i),
                    Direction::Left => (self.width.saturating_sub(pos - offset + 1), i),
                };

                if let Some(buf_idx) = get_index(x, y) {
                    let char_idx = (start_idx + pos - offset) % self.chars.len();
                    // Use emoji head override if set for this column
                    let c = if offset == 0 {
                        if let Some(emoji) = self.emoji_heads[i] {
                            emoji
                        } else {
                            self.chars[char_idx]
                        }
                    } else {
                        self.chars[char_idx]
                    };
                    let color = if offset == 0 {
                        self.head_colors[i]
                    } else if let Some(fade) = &self.body_colors[i].1 {
                        fade[offset - 1]
                    } else {
                        self.body_colors[i].0
                    };
                    // When shading, cells that are too dark to distinguish
                    // from the background become invisible (spaces).
                    let [r, g, b] = color;
                    if self.shading && offset > 0 && (r as u16 + g as u16 + b as u16) < 40 {
                        self.screen_buffer[buf_idx] = Cell::default();
                    } else {
                        self.screen_buffer[buf_idx] = Cell::new(c).color(color);
                    }
                }
            }

            self.positions[i] += 1;
        }

        Ok(())
    }
}

/// Generates a smooth gradient from `base` (index 0) to `shade` (last index).
/// Used for the body trail: index 0 = second character (right after head),
/// last index = tail character (matches background).
pub fn gen_shade_color(base: [u8; 3], shade: [u8; 3], length: u8) -> Vec<[u8; 3]> {
    if length == 0 {
        return Vec::new();
    }
    if length == 1 {
        return vec![base];
    }
    let [br, bg, bb] = base;
    let [sr, sg, sb] = shade;
    let last = (length - 1) as f32;

    (0..length)
        .map(|i| {
            let t = i as f32 / last; // 0.0 at start, 1.0 at end
            let r = (br as f32 * (1.0 - t) + sr as f32 * t) as u8;
            let g = (bg as f32 * (1.0 - t) + sg as f32 * t) as u8;
            let b = (bb as f32 * (1.0 - t) + sb as f32 * t) as u8;
            [r, g, b]
        })
        .collect()
}
