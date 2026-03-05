# feat: Emotive Rain — Agent-Driven Per-Raindrop Visual Expression

## Overview

Give the openclaw AI agent the ability to dynamically alter the matrix rain's visual properties — colors, emoji accents, gradients, and speed — on a per-raindrop basis to express its "inner emotions" during conversation. Changes are smooth and interpolated, never jarring. The rain becomes a living mood ring: green and steady when neutral, warm magenta with sparkle emojis when excited, deep indigo with thought bubbles when contemplative, angry red with warning signs when frustrated. Emojis appear as sparse accents (~10% of strands), not filling the screen — preserving the matrix rain aesthetic while adding expressive flavor. The agent can also send contextual emojis based on the conversation topic (robots when discussing AI, moons for night, plants for nature). The agent controls this via a `mood.update` JSON-RPC notification over the existing WebSocket gateway.

The core insight: rain drops have natural lifecycles. When a drop finishes falling and resets, the new drop can inherit the current emotion palette and emoji accents. This means transitions happen organically — existing drops keep their colors while new drops adopt the target mood — creating a wave-like visual transition for free.

## Problem Statement / Motivation

Right now the matrix rain is a static aesthetic. The user picks colors once (CLI flags or settings panel) and the rain stays that way. The AI agent has no way to express itself visually — all communication is text in the chat panel. This misses the opportunity to make the TUI feel alive and responsive. The rain background occupies 100% of the screen but carries zero semantic information about the conversation.

Imagine: you tell openclaw something exciting, and the rain shifts from green to warm gold. You ask a deep question, and the rain slows to a contemplative indigo. The agent gets frustrated with a tricky task, and red glitch lines crackle through the rain. This creates an ambient, emotional layer of communication that makes the experience feel like talking to something that's *present*.

## Proposed Solution

### Architecture: MoodDirector as Transient Overlay

A new `MoodDirector` struct lives on `App` (not inside `Rain`). It represents the agent's current emotional state as an overlay on top of the user's base settings. The user's CLI/settings choices are always the baseline. Emotion is a delta applied on top.

```
User settings (persist.rs)    Agent emotion (mood.update)
         │                              │
         ▼                              ▼
    Rain::new(cli)              MoodDirector.tick()
         │                              │
         ▼                              ▼
   base_body_color ──────►  lerp(base, target, progress)  ──► Rain.body_colors[i]
   base_head_color ──────►  lerp(base, target, progress)  ──► Rain.head_colors[i]
   base_speed_range ─────►  lerp(base, target, progress)  ──► Rain.speed
   base_chars ───────────►  10% strands get emoji head     ──► Rain column head char override
                             90% strands keep base chars
```

### Data Flow

```
[AI Agent / LLM]
       │
       ▼  JSON-RPC: "mood.update" notification
[Gateway WebSocket Task]  ── parses IncomingFrame::MoodUpdate
       │
       ▼  mpsc channel: GatewayAction::MoodUpdate(MoodState)
[App::process_gateway_actions()]
       │
       ▼  stores in App.mood_director
[MoodDirector]  ── owns per-field Tween<[u8;3]> for colors
       │           ── owns EmojiAccents for sparse emoji overlay (~10% of strands)
       │           ── owns Tween<f32> for speed multiplier
       │
       ▼  called from App.tick() — applies interpolated values to Rain
[Rain fields updated]  ── body_colors, head_colors, speed range modified
       │                   at column reset boundaries
       ▼
[Rain::update() + update_screen_buffer()]
       │
       ▼  screen_buffer: Vec<Cell>
[RainWidget::render()]  ── reads cells, writes to ratatui Buffer (unchanged)
```

### Key Design Decisions

1. **Column-level targets, not cell-level** — matches existing Rain architecture. Each column gets emotion-influenced colors on `reset()`. No per-cell overhead.

2. **Natural lifecycle transitions** — new drops adopt the target palette on spawn. Existing drops keep current colors until they finish. This creates organic wave-like transitions without any explicit animation code in the rain engine.

3. **Oklab color interpolation** — perceptually uniform, no muddy midpoints. Inline ~40 lines of conversion code, no dependency.

4. **Retargetable tweens** — if a new mood arrives mid-transition, snapshot the current interpolated value as the new "from" and start a fresh transition to the new target. No visual discontinuity.

5. **Presets + creative override** — predefined mood-to-visual mappings for common emotions, but also a raw parameter mode for the agent to surprise us.

6. **Sparse emoji accents, not full replacement** — mood emojis appear on only ~10% of strands, scattered among the base characters. This keeps the matrix rain aesthetic while adding expressive accents. Multiple emoji variants per mood ensure visual variety. The agent can also send contextual emojis (robots when discussing AI, moons for night, etc.).

## Technical Approach

### Phase 1: Foundation — MoodDirector + Color Transitions

**Goal**: Agent can send mood updates that smoothly transition rain body and head colors.

#### New file: `src/mood.rs`

```rust
// src/mood.rs

use std::time::{Duration, Instant};

/// Predefined mood presets
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Neutral,
    Curious,
    Excited,
    Contemplative,
    Frustrated,
    Amused,
    Focused,
    Serene,
}

/// Visual parameters derived from a mood
#[derive(Debug, Clone)]
pub struct MoodVisuals {
    pub body_color: [u8; 3],
    pub head_color: [u8; 3],
    pub speed_multiplier: f32,  // 1.0 = unchanged, <1 = faster, >1 = slower
    /// Emoji accents: scattered among base chars at `emoji_density` rate.
    /// Multiple variants for visual variety. None = no emoji accents.
    pub emojis: Option<Vec<char>>,
    /// Fraction of strands that get emoji (0.0-1.0, typically 0.10 = 10%)
    pub emoji_density: f32,
}

impl Mood {
    pub fn visuals(&self) -> MoodVisuals {
        match self {
            Mood::Neutral => MoodVisuals {
                body_color: [0, 255, 0],      // pure green (default)
                head_color: [255, 255, 255],  // white
                speed_multiplier: 1.0,
                emojis: None,                 // no emoji accents
                emoji_density: 0.0,
            },
            Mood::Curious => MoodVisuals {
                body_color: [0, 120, 255],    // cool blue
                head_color: [180, 220, 255],  // light blue
                speed_multiplier: 1.3,        // slightly slower
                emojis: Some(vec!['?', '\u{1F50D}', '\u{1F914}', '\u{1F9D0}', '\u{2753}']),
                // magnifying glass, thinking face, monocle face, question mark
                emoji_density: 0.08,
            },
            Mood::Excited => MoodVisuals {
                body_color: [255, 50, 200],   // magenta-pink
                head_color: [255, 255, 0],    // bright yellow
                speed_multiplier: 0.6,        // faster
                emojis: Some(vec!['\u{2728}', '\u{1F525}', '\u{26A1}', '\u{1F4A5}', '\u{1F389}', '\u{1F680}']),
                // sparkles, fire, lightning, collision, party, rocket
                emoji_density: 0.12,
            },
            Mood::Contemplative => MoodVisuals {
                body_color: [60, 0, 180],     // deep indigo
                head_color: [140, 100, 255],  // lavender
                speed_multiplier: 1.5,        // slower
                emojis: Some(vec!['\u{1F4AD}', '\u{2728}', '\u{1F30C}', '\u{269B}']),
                // thought bubble, sparkles, milky way, atom
                emoji_density: 0.06,
            },
            Mood::Frustrated => MoodVisuals {
                body_color: [255, 60, 0],     // red-orange
                head_color: [255, 200, 0],    // amber
                speed_multiplier: 0.7,        // faster
                emojis: Some(vec!['\u{1F4A2}', '\u{26A0}', '\u{2757}', '\u{1F525}', '\u{1F4A3}']),
                // anger symbol, warning, exclamation, fire, bomb
                emoji_density: 0.10,
            },
            Mood::Amused => MoodVisuals {
                body_color: [255, 180, 50],   // warm gold
                head_color: [255, 255, 100],  // bright yellow
                speed_multiplier: 0.9,
                emojis: Some(vec!['\u{1F602}', '\u{1F604}', '\u{1F60A}', '\u{1F923}', '\u{1F609}', '\u{1F61C}']),
                // laughing faces, winking, etc — variety of smiles
                emoji_density: 0.10,
            },
            Mood::Focused => MoodVisuals {
                body_color: [200, 200, 200],  // near-white
                head_color: [255, 255, 255],  // pure white
                speed_multiplier: 0.8,
                emojis: Some(vec!['\u{1F3AF}', '\u{2699}', '\u{1F4BB}']),
                // target, gear, laptop — clean, minimal
                emoji_density: 0.05,
            },
            Mood::Serene => MoodVisuals {
                body_color: [0, 220, 200],    // cyan-teal
                head_color: [150, 255, 240],  // pastel mint
                speed_multiplier: 1.4,        // slower
                emojis: Some(vec!['\u{1F33F}', '\u{1F33B}', '\u{1F338}', '\u{1F343}', '\u{1F340}', '\u{2618}']),
                // herb, sunflower, cherry blossom, leaf, four-leaf clover, shamrock
                emoji_density: 0.10,
            },
        }
    }
}

/// Incoming mood update from the agent
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MoodUpdate {
    /// Preset mood (optional if custom is provided)
    pub mood: Option<Mood>,
    /// Intensity 0.0-1.0 (scales distance from neutral to target)
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    /// Custom overrides (bypass presets)
    pub custom: Option<CustomVisuals>,
    /// Override transition duration in ms
    pub transition_ms: Option<u64>,
}

fn default_intensity() -> f32 { 1.0 }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CustomVisuals {
    pub body_color: Option<[u8; 3]>,
    pub head_color: Option<[u8; 3]>,
    pub speed_multiplier: Option<f32>,
    /// Agent-chosen contextual emojis — e.g. robots when discussing AI,
    /// moons for night, skulls for spooky topics. Sent as a string of
    /// emoji chars that gets split into individual chars.
    /// Examples: "🤖🦾🧠💻" or "🌙🌑🌕✨🌃"
    pub emojis: Option<String>,
    /// Override emoji density (default: 0.10 = 10% of strands)
    pub emoji_density: Option<f32>,
}

/// Single-value tween with retargeting support
#[derive(Clone)]
pub struct Tween {
    from: [u8; 3],
    to: [u8; 3],
    start: Instant,
    duration: Duration,
}

impl Tween {
    pub fn settled(color: [u8; 3]) -> Self {
        Self {
            from: color,
            to: color,
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }

    pub fn start(from: [u8; 3], to: [u8; 3], duration: Duration) -> Self {
        Self { from, to, start: Instant::now(), duration }
    }

    pub fn retarget(&mut self, new_to: [u8; 3], duration: Duration) {
        self.from = self.current();
        self.to = new_to;
        self.start = Instant::now();
        self.duration = duration;
    }

    pub fn current(&self) -> [u8; 3] {
        if self.duration.is_zero() { return self.to; }
        let t = self.progress();
        lerp_oklab(self.from, self.to, ease_in_out_cubic(t))
    }

    pub fn is_done(&self) -> bool {
        self.start.elapsed() >= self.duration
    }

    fn progress(&self) -> f32 {
        if self.duration.is_zero() { return 1.0; }
        (self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }
}

/// Manages smooth visual transitions driven by agent mood updates
pub struct MoodDirector {
    pub body_tween: Tween,
    pub head_tween: Tween,
    pub speed_tween: SpeedTween,
    pub emoji_accents: EmojiAccents,
    pub current_mood: Option<Mood>,
    pub intensity: f32,
    /// Base colors from user settings (the "neutral" state)
    base_body: [u8; 3],
    base_head: [u8; 3],
    base_speed_range: (u64, u64),
}

#[derive(Clone)]
pub struct SpeedTween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl SpeedTween {
    pub fn settled(mult: f32) -> Self {
        Self { from: mult, to: mult, start: Instant::now(), duration: Duration::ZERO }
    }

    pub fn retarget(&mut self, new_to: f32, duration: Duration) {
        self.from = self.current();
        self.to = new_to;
        self.start = Instant::now();
        self.duration = duration;
    }

    pub fn current(&self) -> f32 {
        if self.duration.is_zero() { return self.to; }
        let t = (self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0);
        let eased = ease_in_out_cubic(t);
        self.from + (self.to - self.from) * eased
    }

    pub fn is_done(&self) -> bool {
        self.start.elapsed() >= self.duration
    }
}

impl MoodDirector {
    pub fn new(base_body: [u8; 3], base_head: [u8; 3], base_speed_range: (u64, u64)) -> Self {
        Self {
            body_tween: Tween::settled(base_body),
            head_tween: Tween::settled(base_head),
            speed_tween: SpeedTween::settled(1.0),
            current_mood: None,
            intensity: 0.0,
            base_body,
            base_head,
            base_speed_range,
        }
    }

    /// Update base settings (called when user changes settings)
    pub fn update_base(&mut self, body: [u8; 3], head: [u8; 3], speed_range: (u64, u64)) {
        self.base_body = body;
        self.base_head = head;
        self.base_speed_range = speed_range;
    }

    /// Apply a mood update from the agent
    pub fn apply_mood(&mut self, update: &MoodUpdate) {
        let duration = Duration::from_millis(update.transition_ms.unwrap_or(2500));
        self.intensity = update.intensity.clamp(0.0, 1.0);
        self.current_mood = update.mood;

        // Resolve target visuals
        let mut visuals = if let Some(mood) = update.mood {
            mood.visuals()
        } else {
            Mood::Neutral.visuals()
        };

        // Apply custom overrides on top of preset
        if let Some(ref custom) = update.custom {
            if let Some(c) = custom.body_color { visuals.body_color = c; }
            if let Some(c) = custom.head_color { visuals.head_color = c; }
            if let Some(s) = custom.speed_multiplier { visuals.speed_multiplier = s; }
            if let Some(ref emoji_str) = custom.emojis {
                visuals.emojis = Some(emoji_str.chars().collect());
            }
            if let Some(d) = custom.emoji_density {
                visuals.emoji_density = d;
            }
        }

        // Apply intensity: lerp between base (neutral) and target
        let target_body = lerp_oklab(self.base_body, visuals.body_color, self.intensity);
        let target_head = lerp_oklab(self.base_head, visuals.head_color, self.intensity);
        let target_speed = 1.0 + (visuals.speed_multiplier - 1.0) * self.intensity;

        // Retarget tweens (handles mid-transition seamlessly)
        self.body_tween.retarget(target_body, duration);
        self.head_tween.retarget(target_head, duration);
        self.speed_tween.retarget(target_speed.clamp(0.3, 3.0), duration);

        // Update emoji accents
        let emoji_chars = visuals.emojis.unwrap_or_default();
        self.emoji_accents.set_target(emoji_chars, visuals.emoji_density, duration.as_secs_f32());
    }

    /// Get current interpolated body color
    pub fn body_color(&self) -> [u8; 3] {
        self.body_tween.current()
    }

    /// Get current interpolated head color
    pub fn head_color(&self) -> [u8; 3] {
        self.head_tween.current()
    }

    /// Get current speed multiplier
    pub fn speed_multiplier(&self) -> f32 {
        self.speed_tween.current()
    }

    /// Whether any transition is actively in progress
    pub fn is_transitioning(&self) -> bool {
        !self.body_tween.is_done() || !self.head_tween.is_done() || !self.speed_tween.is_done()
    }
}

// --- Oklab color interpolation (inline, no dependency) ---

fn srgb_to_linear(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> u8 {
    let v = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

struct Oklab { l: f32, a: f32, b: f32 }

fn rgb_to_oklab(rgb: [u8; 3]) -> Oklab {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
    let l_ = l.cbrt(); let m_ = m.cbrt(); let s_ = s.cbrt();
    Oklab {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

fn oklab_to_rgb(lab: Oklab) -> [u8; 3] {
    let l_ = lab.l + 0.3963377774 * lab.a + 0.2158037573 * lab.b;
    let m_ = lab.l - 0.1055613458 * lab.a - 0.0638541728 * lab.b;
    let s_ = lab.l - 0.0894841775 * lab.a - 1.2914855480 * lab.b;
    let l = l_ * l_ * l_; let m = m_ * m_ * m_; let s = s_ * s_ * s_;
    [
        linear_to_srgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        linear_to_srgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        linear_to_srgb(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    ]
}

pub fn lerp_oklab(from: [u8; 3], to: [u8; 3], t: f32) -> [u8; 3] {
    if t <= 0.0 { return from; }
    if t >= 1.0 { return to; }
    let a = rgb_to_oklab(from);
    let b = rgb_to_oklab(to);
    oklab_to_rgb(Oklab {
        l: a.l + (b.l - a.l) * t,
        a: a.a + (b.a - a.a) * t,
        b: a.b + (b.b - a.b) * t,
    })
}

fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}
```

#### Modify: `src/gateway/protocol.rs`

Add `MoodUpdate` variant to `IncomingFrame` and parse `"mood.update"` method:

```rust
// Add to IncomingFrame enum:
MoodUpdate(crate::mood::MoodUpdate),

// Add to parse() match:
"mood.update" => {
    if let Some(params) = frame.params {
        if let Ok(update) = serde_json::from_value(params) {
            return IncomingFrame::MoodUpdate(update);
        }
    }
    IncomingFrame::Unknown(text.to_string())
}
```

#### Modify: `src/gateway/mod.rs`

Add `MoodUpdate` to `GatewayAction`:

```rust
pub enum GatewayAction {
    Connected,
    Disconnected(String),
    ChatDelta(String),
    ChatComplete(String),
    Error(String),
    MoodUpdate(crate::mood::MoodUpdate),  // new
}
```

Wire in the gateway task's message handler.

#### Modify: `src/app.rs`

- Add `mood_director: MoodDirector` field to `App`
- In `process_gateway_actions()`, handle `GatewayAction::MoodUpdate`
- In `tick()`, apply `mood_director` colors to rain columns on reset

#### Modify: `src/rain/mod.rs`

- Add `pub fn set_column_colors(&mut self, body: [u8; 3], head: [u8; 3])` method
- Called from `App::tick()` — when a column resets, it uses the current mood color instead of the original CLI color
- Add `pub fn set_speed_range(&mut self, range: std::ops::Range<u64>)` to allow dynamic speed changes

#### Wire `mood.update` JSON-RPC Protocol

Example payloads:

```json
// Preset mood
{"jsonrpc": "3.0", "method": "mood.update", "params": {
    "mood": "excited",
    "intensity": 0.8
}}

// Creative override with contextual emojis (discussing robots)
{"jsonrpc": "3.0", "method": "mood.update", "params": {
    "mood": "excited",
    "custom": {
        "body_color": [255, 200, 50],
        "head_color": [255, 255, 200],
        "speed_multiplier": 0.7,
        "emojis": "\u{1F916}\u{1F9BE}\u{1F9E0}\u{1F4BB}\u{2699}",
        "emoji_density": 0.12
    },
    "intensity": 1.0
}}

// Reset to neutral
{"jsonrpc": "3.0", "method": "mood.update", "params": {
    "mood": "neutral",
    "intensity": 0.0
}}
```

#### Tasks

- [ ] Create `src/mood.rs` with `Mood`, `MoodUpdate`, `MoodVisuals`, `MoodDirector`, `Tween`, Oklab interpolation, and easing functions
- [ ] Add `MoodUpdate` variant to `IncomingFrame` in `src/gateway/protocol.rs`
- [ ] Add `"mood.update"` parsing to `IncomingFrame::parse()` in `src/gateway/protocol.rs`
- [ ] Add `MoodUpdate` variant to `GatewayAction` in `src/gateway/mod.rs`
- [ ] Wire `IncomingFrame::MoodUpdate` → `GatewayAction::MoodUpdate` in gateway task
- [ ] Add `mood_director: MoodDirector` to `App` struct in `src/app.rs`
- [ ] Handle `GatewayAction::MoodUpdate` in `App::process_gateway_actions()`
- [ ] Add `set_column_colors()` and `set_speed_range()` methods to `Rain` in `src/rain/mod.rs`
- [ ] Modify `Rain::reset()` to accept optional color overrides (or read from new fields)
- [ ] In `App::tick()`, apply `mood_director.body_color()` / `head_color()` to rain on column resets
- [ ] Add `mod mood;` to `src/main.rs`
- [ ] Write unit tests for `Tween` (settled, retarget mid-transition, completion)
- [ ] Write unit tests for `lerp_oklab` (endpoints, midpoint, edge cases)
- [ ] Write unit test for `MoodDirector::apply_mood` with preset and custom

### Phase 2: Sparse Emoji Accents

**Goal**: Mood emojis appear as scattered accents on ~10% of rain strands, not replacing the entire character set. The agent can also send contextual emojis relevant to the conversation topic.

#### Design: Sparse Emoji Overlay

Emojis are **accent characters**, not a full character set replacement. The base rain characters (binary, katakana, etc.) remain the foundation. A small percentage of strands get an emoji character mixed into their trail — typically as the head character.

```rust
/// Manages sparse emoji accents on rain strands
pub struct EmojiAccents {
    /// Current emoji pool (multiple variants for variety)
    current_emojis: Vec<char>,
    /// Target emoji pool (what we're transitioning to)
    target_emojis: Vec<char>,
    /// Current density: fraction of strands that get emoji (0.0-1.0)
    current_density: f32,
    /// Target density
    target_density: f32,
    /// Transition progress (0.0 = all current, 1.0 = all target)
    progress: f32,
    /// Progress increment per tick (~0.0125 for 4s transition at 20fps)
    speed: f32,
}

impl EmojiAccents {
    /// Called when a column resets: should this strand get an emoji?
    pub fn should_accent(&self, rng: &mut Random) -> bool {
        let density = self.current_density + (self.target_density - self.current_density) * self.progress;
        rng.random_float() < density
    }

    /// Pick a random emoji from the blended pool
    pub fn sample(&self, rng: &mut Random) -> char {
        // During transition, probabilistically pick from old or new set
        let pool = if self.progress >= 1.0 || self.current_emojis.is_empty() {
            &self.target_emojis
        } else if self.target_emojis.is_empty() || rng.random_float() > self.progress {
            &self.current_emojis
        } else {
            &self.target_emojis
        };
        if pool.is_empty() { return '#'; }
        pool[rng.random_range(0..pool.len())]
    }

    pub fn tick(&mut self) {
        if self.progress < 1.0 {
            self.progress = (self.progress + self.speed).min(1.0);
            if self.progress >= 1.0 {
                self.current_emojis = self.target_emojis.clone();
                self.current_density = self.target_density;
            }
        }
    }

    pub fn set_target(&mut self, emojis: Vec<char>, density: f32, transition_secs: f32) {
        self.current_emojis = self.target_emojis.clone();
        self.current_density = self.current_density + (self.target_density - self.current_density) * self.progress;
        self.target_emojis = emojis;
        self.target_density = density.clamp(0.0, 0.25); // cap at 25% to preserve rain aesthetic
        self.progress = 0.0;
        self.speed = if transition_secs > 0.0 { 0.05 / transition_secs } else { 1.0 };
    }
}
```

#### How it works in the Rain engine

When a column calls `reset()`:
1. Ask `emoji_accents.should_accent(rng)` — rolls against current density (~10%)
2. If yes, the **head character** of this strand is replaced with `emoji_accents.sample(rng)`
3. The rest of the strand's characters remain from the base `chars` array
4. This means at any time, ~10% of visible rain heads are emojis, scattered randomly

This is lightweight — no grid restructuring, no width changes, just one character override per accented strand.

#### Contextual emojis from the agent

The agent can send topic-relevant emojis via the `custom.emojis` field in `mood.update`:

```json
// Discussing AI/robots
{"method": "mood.update", "params": {
    "mood": "excited",
    "intensity": 0.8,
    "custom": { "emojis": "\u{1F916}\u{1F9BE}\u{1F9E0}\u{1F4BB}\u{2699}" }
}}

// Talking about space/night
{"method": "mood.update", "params": {
    "mood": "contemplative",
    "custom": { "emojis": "\u{1F319}\u{1F311}\u{1F315}\u{2728}\u{1F30C}\u{1F6F8}" }
}}

// Discussing nature
{"method": "mood.update", "params": {
    "mood": "serene",
    "custom": { "emojis": "\u{1F333}\u{1F33F}\u{1F33B}\u{1F338}\u{1F98B}\u{1F426}" }
}}
```

When `custom.emojis` is provided, it overrides the preset's emoji set. When absent, the preset's default emojis are used. This gives the agent creative freedom while keeping sensible defaults.

#### MVP vs Full version

**MVP (Phase 2)**: Emoji accents work with preset emoji sets per mood + agent can send a custom emoji string. Head-character-only replacement (simple, no width issues).

**Future enhancement**: Emoji characters could appear at multiple positions in the trail (not just head), with trailing emojis gradually fading. But MVP is head-only — simple and effective.

#### Tasks

- [ ] Add `EmojiAccents` struct to `src/mood.rs`
- [ ] Add `emoji_accents: EmojiAccents` to `MoodDirector`
- [ ] When `apply_mood()` is called, extract emojis from preset or `custom.emojis` and call `set_target()`
- [ ] Parse `custom.emojis` string into `Vec<char>` (split on char boundaries)
- [ ] Add `emoji_override: Option<char>` field to `Rain` column state (or pass via new `reset()` parameter)
- [ ] In `Rain::reset()`, when emoji override is set, use it as head character for the strand
- [ ] In `App::tick()`, when a column resets, consult `mood_director.emoji_accents.should_accent()` and if true, pass `emoji_accents.sample()` as the head char override
- [ ] Tick `emoji_accents.progress` each frame in `MoodDirector`
- [ ] Cap emoji density at 25% to preserve the matrix rain aesthetic
- [ ] Write unit test for `EmojiAccents` density rolloff and sampling distribution

### Phase 3: Status Bar + Effect Integration

**Goal**: Show current mood in status bar. Integrate mood with the effects system.

#### Status bar mood indicator

Add a mood indicator between the mode and connection status spans in `App::draw_status_bar()`:

```rust
// In draw_status_bar:
if let Some(mood) = self.mood_director.current_mood {
    let mood_str = format!(" {:?} ", mood).to_uppercase();
    let mood_color = to_color(self.mood_director.body_color());
    spans.push(Span::styled(mood_str, Style::default().fg(Color::Black).bg(mood_color)));
    spans.push(Span::raw(" "));
}
```

#### Mood-influenced effects

When the user sends a message, the `EffectManager::trigger()` call could select the effect type based on the current mood:

| Mood | Effect |
|------|--------|
| Neutral | Burst (default) |
| Excited | Burst (larger radius) |
| Frustrated | Glitch |
| Contemplative | Dissolve (slower) |
| Amused | Burst with emoji chars |

#### Tasks

- [ ] Add mood indicator to `App::draw_status_bar()` in `src/app.rs`
- [ ] Add `trigger_with_mood(x, y, mood: Option<Mood>)` to `EffectManager` in `src/effects/mod.rs`
- [ ] Wire mood into the Enter-key handler in `App::handle_typing_key()`

### Phase 4: Rainbow Cycling for "Amused" Mood

**Goal**: The `amused` mood creates a rainbow wave effect across columns.

The `MoodDirector` detects when the active mood is `Amused` and enters a special cycling mode instead of a fixed-target tween. Each tick, the body color target shifts by a small hue increment. Each column is phase-offset so the rainbow sweeps across the screen.

```rust
// In MoodDirector, when mood is Amused:
// Each tick, advance hue_offset by ~2 degrees
// Column i gets: hue = (hue_offset + i * column_hue_step) % 360
// Convert HSL(hue, 0.9, 0.6) → RGB → apply as body color
```

This is a special case — instead of a one-shot tween to a fixed target, it is a continuous animation. The `MoodDirector` can have an `active_animation: Option<ContinuousAnimation>` field that overrides the body tween when active.

#### Tasks

- [ ] Add `ContinuousAnimation` enum to `src/mood.rs` with `RainbowCycle { hue_offset: f32, speed: f32 }`
- [ ] Add `active_animation` field to `MoodDirector`
- [ ] In `MoodDirector`, when mood is `Amused`, start `RainbowCycle` instead of a fixed tween
- [ ] Add `body_color_for_column(column: usize, total_columns: usize) -> [u8; 3]` method
- [ ] Modify `App::tick()` to use per-column color from `body_color_for_column()` when animation is active
- [ ] Add HSL-to-RGB helper (or Oklch for perceptual uniformity)

### Phase 5: Reconnection + Settings Interaction Polish

**Goal**: Clean handling of edge cases where mood state interacts with gateway disconnection and settings panel.

#### Reconnection behavior

- On `GatewayAction::Disconnected`: mood state is preserved (the rain keeps its current colors)
- On `GatewayAction::Connected`: mood state is NOT reset. The agent will send a new `mood.update` if it wants to set the mood.
- No automatic "reset to neutral" on reconnect — the visual state gracefully carries over.

#### Settings panel interaction

- Opening settings (`Ctrl+S`): mood state pauses (rain pauses anyway in Settings mode)
- Closing settings: `Rain::new()` rebuilds rain with user's new settings. `MoodDirector::update_base()` is called with the new base colors. If an emotion was active, the tweens continue from the new base toward the same target.
- This means the user's settings changes are respected as the new "neutral" baseline, and the emotion overlay recomputes relative to it.

#### Tasks

- [ ] In settings-close handler, call `mood_director.update_base(new_body, new_head, new_speed_range)`
- [ ] Verify mood is not reset on `GatewayAction::Connected` or `Disconnected`
- [ ] Add `update_base()` method to `MoodDirector` that recomputes tween targets relative to new base
- [ ] In `rebuild_rain()`, preserve and reapply mood state

## Mood Preset Reference

| Mood | Body Color | Head Color | Speed | Emoji Accents (~density) | Effect |
|------|-----------|-----------|-------|-------------------------|--------|
| Neutral | `[0, 255, 0]` green | `[255, 255, 255]` white | 1.0x | none | Burst |
| Curious | `[0, 120, 255]` blue | `[180, 220, 255]` light blue | 1.3x (slower) | ? \u{1F50D} \u{1F914} \u{1F9D0} (~8%) | Dissolve |
| Excited | `[255, 50, 200]` magenta | `[255, 255, 0]` yellow | 0.6x (faster) | \u{2728} \u{1F525} \u{26A1} \u{1F4A5} \u{1F389} \u{1F680} (~12%) | Burst (large) |
| Contemplative | `[60, 0, 180]` indigo | `[140, 100, 255]` lavender | 1.5x (slower) | \u{1F4AD} \u{2728} \u{1F30C} \u{269B} (~6%) | Dissolve (slow) |
| Frustrated | `[255, 60, 0]` red-orange | `[255, 200, 0]` amber | 0.7x (faster) | \u{1F4A2} \u{26A0} \u{2757} \u{1F525} \u{1F4A3} (~10%) | Glitch |
| Amused | rainbow cycle | `[255, 255, 100]` yellow | 0.9x | \u{1F602} \u{1F604} \u{1F60A} \u{1F923} \u{1F609} \u{1F61C} (~10%) | Burst |
| Focused | `[200, 200, 200]` silver | `[255, 255, 255]` white | 0.8x | \u{1F3AF} \u{2699} \u{1F4BB} (~5%) | Burst |
| Serene | `[0, 220, 200]` teal | `[150, 255, 240]` mint | 1.4x (slower) | \u{1F33F} \u{1F33B} \u{1F338} \u{1F343} \u{1F340} \u{2618} (~10%) | Dissolve |

**Contextual emojis** (agent-chosen, override preset emojis):

| Topic | Example Emojis | Triggered by |
|-------|---------------|-------------|
| AI/Robots | \u{1F916} \u{1F9BE} \u{1F9E0} \u{1F4BB} \u{2699} | Agent sends `custom.emojis` |
| Space/Night | \u{1F319} \u{1F311} \u{1F315} \u{2728} \u{1F30C} \u{1F6F8} | Agent sends `custom.emojis` |
| Nature | \u{1F333} \u{1F33F} \u{1F33B} \u{1F98B} \u{1F426} | Agent sends `custom.emojis` |
| Music | \u{1F3B5} \u{1F3B6} \u{1F3A4} \u{1F3B8} | Agent sends `custom.emojis` |
| Money/Finance | \u{1F4B0} \u{1F4B8} \u{1F4C8} \u{1F4B9} | Agent sends `custom.emojis` |
| Love | \u{2764} \u{1F495} \u{1F496} \u{1F49C} \u{1F49A} | Agent sends `custom.emojis` |

## Acceptance Criteria

### Functional Requirements

- [ ] Agent can send `mood.update` JSON-RPC notifications over WebSocket
- [ ] Rain colors transition smoothly (not instantly) when mood changes
- [ ] Transitions use Oklab interpolation with ease-in-out-cubic easing
- [ ] New drops adopt target mood palette; existing drops keep current colors until they reset
- [ ] Rapid mood changes mid-transition produce no visual discontinuity (retargeting)
- [ ] All 8 mood presets produce distinct, visually coherent rain aesthetics
- [ ] `intensity` parameter scales from neutral (0.0) to full mood color (1.0)
- [ ] Creative override allows arbitrary `[r,g,b]` colors, speed multipliers, and custom emojis
- [ ] Mood emojis appear as sparse accents (~10% of strands), not filling the entire screen
- [ ] Multiple emoji variants per mood ensure visual variety (no repetitive single-emoji walls)
- [ ] Agent can send contextual emojis via `custom.emojis` field (robots for AI, moons for night, etc.)
- [ ] Emoji density is capped at 25% to preserve the matrix rain aesthetic
- [ ] Emoji transitions blend probabilistically (old and new sets mix during transition)
- [ ] Amused mood produces rainbow cycling effect
- [ ] Status bar shows current mood name and color
- [ ] Settings panel changes update the mood baseline without discarding active emotion
- [ ] Offline mode works unchanged (no mood updates, no crashes)
- [ ] Sending `mood: neutral, intensity: 0.0` smoothly transitions back to user settings

### Non-Functional Requirements

- [ ] Tick loop stays under 50ms total (20fps) during mood transitions
- [ ] No per-tick heap allocations in the mood interpolation hot path
- [ ] Color quantization for 256-color terminals is automatic (existing `to_color()` pipeline)
- [ ] Speed multiplier clamped to `[0.3, 3.0]` range (safety against seizure-inducing speeds)
- [ ] Color brightness floor of RGB sum >= 20 (prevent invisible rain from agent)

## Dependencies & Risks

**Dependencies:**
- Server-side `mood.update` emission — the agent backend must be modified to send these notifications. This plan covers the client only.
- The JSON-RPC schema must be agreed with the server team.

**Risks:**
- **256-color terminals**: Smooth Oklab transitions will appear stepped due to color quantization. Mitigated by the existing fallback pipeline; visually acceptable but not as smooth.
- **Character width transitions**: Cross-width transitions (binary→emoji) are inherently jarring due to grid restructuring. Mitigated by treating these as full resets with a dissolve effect.
- **Agent abuse**: A malicious/buggy agent could send seizure-inducing rapid mood changes or invisible colors. Mitigated by speed clamping and brightness floor.

## References

### Internal References

- Rain engine: `src/rain/mod.rs:112-151` (Rain struct), `src/rain/mod.rs:291-375` (reset + update_screen_buffer)
- Color pipeline: `src/rain/widget.rs:13-50` (supports_truecolor, rgb_to_ansi256, to_color)
- Gateway protocol: `src/gateway/protocol.rs:48-118` (IncomingFrame enum + parse)
- Gateway channels: `src/gateway/mod.rs:22-49` (GatewayAction + spawn_gateway)
- Effects system: `src/effects/mod.rs` (EffectManager, EffectKind)
- App tick loop: `src/app.rs:93-99` (tick), `src/app.rs:234-265` (process_gateway_actions)
- Settings rebuild: `src/app.rs:186-203` (handle_settings_key close path)
- Char groups: `src/cli.rs:93-277` (Grouping enum + FromStr)

### External References

- [Oklab: A perceptual color space](https://bottosson.github.io/posts/oklab/)
- [Easing Functions Cheat Sheet](https://easings.net/)
- [Plutchik's Wheel of Emotions](https://bricxlabs.com/blogs/plutchik-s-wheel-of-emotions)
- [Ratatui rendering internals](https://ratatui.rs/concepts/rendering/under-the-hood/)
- [tachyonfx: ratatui effects library](https://github.com/ratatui/tachyonfx)
