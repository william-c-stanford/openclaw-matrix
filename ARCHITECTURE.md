# openclaw-mood — Architecture & Technical Documentation

**A Matrix rain TUI chat client for openclaw.**

Terminal-native interface that renders a real-time Matrix digital rain animation while providing a chat overlay for communicating with an AI agent over WebSocket. The agent can dynamically alter the rain's visual properties — colors, emojis, speed, gradients — to express emotional state, creating a living, responsive visual experience.

Built in Rust with [ratatui](https://ratatui.rs) for TUI rendering, [crossterm](https://github.com/crossterm-rs/crossterm) for terminal I/O, and [tokio](https://tokio.rs) for async networking.

**4,303 lines of Rust across 24 source files.**

---

## Table of Contents

- [Quick Start](#quick-start)
- [Project Structure](#project-structure)
- [Architecture Overview](#architecture-overview)
- [Module Reference](#module-reference)
  - [main.rs — Entry Point & Event Loop](#mainrs--entry-point--event-loop)
  - [app.rs — Application State Machine](#apprs--application-state-machine)
  - [rain/ — Matrix Rain Engine](#rain--matrix-rain-engine)
  - [mood.rs — Emotive Rain System](#moodrs--emotive-rain-system)
  - [chat/ — Chat Message State & Rendering](#chat--chat-message-state--rendering)
  - [input/ — Text Input State & Rendering](#input--text-input-state--rendering)
  - [effects/ — Visual Effects](#effects--visual-effects)
  - [gateway/ — WebSocket Gateway Client](#gateway--websocket-gateway-client)
  - [settings/ — In-App Settings Menu](#settings--in-app-settings-menu)
  - [persist.rs — Settings Persistence](#persistrs--settings-persistence)
  - [cli.rs — CLI Argument Parsing](#clirs--cli-argument-parsing)
  - [config.rs & theme.rs — Legacy Configuration](#configrs--themers--legacy-configuration)
- [Data Flow](#data-flow)
- [Gateway Protocol](#gateway-protocol)
- [Mood System Deep Dive](#mood-system-deep-dive)
- [Rendering Pipeline](#rendering-pipeline)
- [Keybindings](#keybindings)
- [Configuration](#configuration)
- [Testing](#testing)
- [Dependencies](#dependencies)

---

## Quick Start

```bash
# Screensaver mode (no server needed)
cargo run -- --offline

# With gateway connection
OPENCLAW_GATEWAY_URL=ws://localhost:3001/ws cargo run

# With custom visuals
cargo run -- --offline --shade -C "0,200,255" -H "#FF00FF" -g classic
```

---

## Project Structure

```
openclaw-mood/
├── Cargo.toml                 # Dependencies & build config
├── plans/
│   └── emotive-rain.md        # Feature plan for the mood system
└── src/
    ├── main.rs                # Entry point, tokio runtime, event loop (170 lines)
    ├── app.rs                 # Application state machine, draw orchestration (420 lines)
    ├── cli.rs                 # CLI arg definitions via clap (472 lines)
    ├── config.rs              # Legacy Config struct (unused by main path) (128 lines)
    ├── theme.rs               # Legacy Theme struct (unused by main path) (53 lines)
    ├── persist.rs             # Settings persistence to TOML (91 lines)
    ├── mood.rs                # Emotive rain: moods, tweens, Oklab, emojis (587 lines)
    ├── test.rs                # Snapshot & unit tests (148 lines)
    ├── rain/
    │   ├── mod.rs             # Core rain simulation engine (474 lines)
    │   ├── widget.rs          # Ratatui StatefulWidget for rain (119 lines)
    │   ├── characters.rs      # Character sets (katakana, ASCII, digits) (50 lines)
    │   └── column.rs          # Per-column rain state (legacy, unused by main) (107 lines)
    ├── chat/
    │   ├── mod.rs             # Chat message state (81 lines)
    │   └── widget.rs          # Chat overlay renderer with word wrap (143 lines)
    ├── input/
    │   ├── mod.rs             # Text input buffer with cursor (84 lines)
    │   └── widget.rs          # Input box renderer (83 lines)
    ├── effects/
    │   ├── mod.rs             # Effect manager & rendering (214 lines)
    │   ├── burst.rs           # Radial burst effect (49 lines)
    │   ├── dissolve.rs        # Text-to-rain dissolve particles (74 lines)
    │   └── glitch.rs          # Horizontal glitch displacement (44 lines)
    ├── gateway/
    │   ├── mod.rs             # WebSocket task, reconnection logic (173 lines)
    │   ├── protocol.rs        # JSON-RPC frame parsing & building (144 lines)
    │   ├── config.rs          # Gateway URL/token resolution (45 lines)
    │   └── device.rs          # Ed25519 device identity (87 lines)
    └── settings/
        ├── mod.rs             # Settings state with cycle-through values (167 lines)
        └── widget.rs          # Full-screen settings UI (96 lines)
```

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                     main.rs                              │
│  tokio runtime → event loop (50ms tick + keyboard events)│
└──────────┬───────────────────────────┬───────────────────┘
           │                           │
    tick every 50ms              keyboard events
           │                           │
           ▼                           ▼
┌──────────────────────────────────────────────────────────┐
│                      App (app.rs)                        │
│  State machine: Viewing | Typing | Settings | Exiting    │
│                                                          │
│  ┌─────────────┐ ┌────────────┐ ┌──────────────────────┐│
│  │ MoodDirector│ │ ChatState  │ │    Rain<1024>        ││
│  │  (mood.rs)  │ │ (chat/)    │ │    (rain/)           ││
│  │             │ │            │ │                      ││
│  │ body_tween  │ │ messages[] │ │ screen_buffer[]      ││
│  │ head_tween  │ │ streaming  │ │ positions/windows    ││
│  │ speed_tween │ │ scroll     │ │ override colors      ││
│  │ emoji_acnts │ │            │ │ emoji heads          ││
│  └──────┬──────┘ └────────────┘ └──────────────────────┘│
│         │ colors/emojis                                  │
│         └──────────────────────────────────▶ Rain        │
│                                                          │
│  ┌─────────────┐ ┌────────────┐ ┌──────────────────────┐│
│  │ InputState  │ │EffectMgr   │ │  SettingsState       ││
│  │ (input/)    │ │(effects/)  │ │  (settings/)         ││
│  └─────────────┘ └────────────┘ └──────────────────────┘│
└──────────────────────────────────────────────────────────┘
           │
    gateway_tx / gateway_rx (mpsc channels)
           │
           ▼
┌──────────────────────────────────────────────────────────┐
│              Gateway Task (gateway/mod.rs)                │
│  Tokio task: WebSocket → JSON-RPC → GatewayAction        │
│  Auto-reconnect with exponential backoff                 │
│  Ed25519 challenge-response auth                         │
└──────────────────────────────────────────────────────────┘
           │
           ▼
    Remote openclaw agent (WebSocket server)
```

**Key design principle:** The app runs a single-threaded render loop at 20 fps (50ms ticks). All networking is handled by an isolated tokio task that communicates via mpsc channels. The mood system overlays agent-driven visuals without disrupting the rain engine's internal state machine.

---

## Module Reference

### main.rs — Entry Point & Event Loop

**File:** `src/main.rs` (170 lines)

The entry point parses CLI args, applies config file overrides and persisted settings, then boots a tokio runtime.

**Key functions:**

| Function | Description |
|----------|-------------|
| `main()` | Parse CLI, load config, start tokio runtime |
| `update_settings_with_config()` | Merge `~/.config/rusty-rain/config.toml` values into CLI settings |
| `async_main()` | Initialize ratatui terminal, create `App`, run event loop |
| `run_app()` | Core loop: `tokio::select!` on 50ms tick interval + terminal events |

**Event loop structure:**
```
loop {
    select! {
        tick_interval => app.tick() + app.process_gateway_actions()
        terminal_event => app.handle_key() or app.rebuild_rain()
    }
    if exiting: break
    terminal.draw(app.draw)
}
```

**Settings priority:** CLI args > environment vars > persisted settings (`settings.toml`) > config file (`config.toml`) > defaults.

---

### app.rs — Application State Machine

**File:** `src/app.rs` (420 lines)

Central application struct that owns all state and orchestrates rendering.

**App modes:**

| Mode | Description | Input handling |
|------|-------------|----------------|
| `Viewing` | Rain animation, no input box | `q`/`Esc` quit, `i`/`/` enter typing, `m` cycle moods, `Ctrl+S` settings, arrows scroll |
| `Typing` | Input box visible, typing messages | `Esc` back to viewing, `Enter` send, standard text editing |
| `Settings` | Full-screen settings overlay, rain paused | `Esc`/`Ctrl+S` apply & close, arrows navigate, left/right change values |
| `Exiting` | Terminal cleanup in progress | N/A |

**App struct fields:**

```rust
pub struct App {
    pub mode: AppMode,
    pub rain: Rain<1024>,           // Rain simulation (1024-char pool)
    pub settings: cli::Cli,         // Current settings
    pub bg_color: Option<(u8,u8,u8)>,
    pub chat: ChatState,            // Messages + streaming
    pub input: InputState,          // Text buffer + cursor
    pub connection_status: ConnectionStatus,
    pub gateway_tx: Option<Sender<GatewayCommand>>,
    pub gateway_rx: Option<Receiver<GatewayAction>>,
    pub settings_state: Option<SettingsState>,
    pub effects: EffectManager,
    pub mood_director: MoodDirector,
    pub term_width: u16,
    pub term_height: u16,
}
```

**Tick cycle (`App::tick`):**
1. Tick mood transitions (MoodDirector)
2. If mood active or transitioning: push override colors to Rain
3. If emoji accents active: sync emoji pool/density to Rain
4. Advance rain simulation (`rain.update()` + `rain.update_screen_buffer()`)
5. Tick visual effects

**Draw layers (back to front):**
1. Rain background (StatefulWidget)
2. Chat messages (overlay, left 45% of screen)
3. Effects overlay (burst/dissolve/glitch)
4. Input box (bottom-left, only in Typing mode)
5. Status bar (bottom row: mode, mood indicator, connection, keyhints)

---

### rain/ — Matrix Rain Engine

**Files:** `src/rain/mod.rs` (474 lines), `src/rain/widget.rs` (119 lines)

The heart of the visual. A columnar rain simulation using pre-allocated character pools and per-column timing.

**`Rain<const LENGTH: usize>` struct:**

The `LENGTH` const generic sizes the character pool (always 1024 in production). Each column has independent:
- **start index** into the shared character array
- **window size** (trail length)
- **position** (current head position, incrementing each update)
- **timing** (Instant + Duration, controls per-column speed)
- **body color** (base + optional shade gradient)
- **head color**
- **direction** (Up/Down/Left/Right)
- **emoji head override** (Option<char>, set by mood system)

**Update cycle:**
1. `update()` — Check timing for each column, push ready columns to `queue`
2. `update_screen_buffer()` — For each queued column:
   - If past end of trail: clear tail cell
   - If finished (past screen + window): `reset()` the column
   - Otherwise: write head + body cells with colors

**Column reset (`reset()`):**
- New random speed, start position, window size
- Apply mood override colors (if set by MoodDirector)
- Roll for emoji head accent (probabilistic based on `emoji_density`)

**Mood integration fields:**
```rust
override_body_color: Option<[u8; 3]>,  // Set by MoodDirector via App
override_head_color: Option<[u8; 3]>,  // Applied on column reset
emoji_heads: Vec<Option<char>>,         // Per-column emoji override
emoji_pool: Vec<char>,                  // Available emojis
emoji_density: f32,                     // 0.0-0.25 fraction
```

**RainWidget (widget.rs):**

A `StatefulWidget` implementation that maps Rain's logical `screen_buffer` to ratatui's `Buffer`. Handles:
- True color vs 256-color fallback (checks `$COLORTERM`)
- RGB → xterm-256 conversion for older terminals
- Background color fill
- Character width scaling (double-width emoji support)

---

### mood.rs — Emotive Rain System

**File:** `src/mood.rs` (587 lines)

The most novel feature: allows an AI agent to dynamically alter the rain's visual properties to express emotional state. All transitions are smooth — no jarring switches.

**Mood presets (8 emotions):**

| Mood | Body Color | Head Color | Speed | Emojis | Density |
|------|-----------|------------|-------|--------|---------|
| Neutral | green (0,255,0) | white | 1.0x | none | 0% |
| Curious | blue (0,120,255) | light blue | 1.3x (slower) | ?, magnifier, thinking, monocle | 8% |
| Excited | magenta (255,50,200) | yellow | 0.6x (faster) | sparkles, fire, lightning, party | 12% |
| Contemplative | indigo (60,0,180) | purple | 1.5x (slower) | thought bubble, galaxy, atom | 6% |
| Frustrated | red-orange (255,60,0) | amber | 0.7x (faster) | anger, warning, exclamation, bomb | 10% |
| Amused | warm orange (255,180,50) | warm yellow | 0.9x | laughing faces, wink | 10% |
| Focused | silver (200,200,200) | white | 0.8x | target, gear, laptop | 5% |
| Serene | teal (0,220,200) | mint | 1.4x (slower) | leaf, sunflower, cherry blossom | 10% |

*Speed multiplier > 1.0 = slower drops, < 1.0 = faster drops (it scales the timing interval).*

**Core components:**

**`Tween`** — Retargetable color interpolation:
- Stores `from`, `to`, `start` (Instant), `duration`
- `retarget()` snapshots current interpolated value as new `from`, sets new `to`
- Interpolation via Oklab color space with ease-in-out-cubic easing
- Handles mid-transition retargeting seamlessly (no jumps)

**`SpeedTween`** — Same pattern for f32 speed multiplier.

**`EmojiAccents`** — Manages sparse emoji transitions:
- Maintains `current_emojis` and `target_emojis` pools
- Progress-based blending: during transition, merges both pools
- Density interpolates linearly between current and target
- Pool assignment happens per-column at natural lifecycle boundaries (column reset)

**`MoodDirector`** — Orchestrates all visual transitions:
- Owns body/head/speed tweens and emoji accents
- `apply_mood(update)` resolves preset visuals, applies custom overrides, factors in intensity, retargets all tweens
- `tick()` advances emoji transitions
- Color/speed queries return current interpolated values
- Maintains `base_body`/`base_head` (user's chosen settings as baseline)

**Oklab color interpolation** — Inline implementation (no crate dependency):
- `srgb_to_linear()` / `linear_to_srgb()` — gamma conversion
- `rgb_to_oklab()` / `oklab_to_rgb()` — full Oklab transform
- `lerp_oklab()` — perceptually uniform interpolation
- Prevents the "muddy brown" problem of naive RGB lerping

**`MoodUpdate`** — Incoming JSON payload:
```rust
pub struct MoodUpdate {
    pub mood: Option<Mood>,         // Preset name (or None for custom-only)
    pub intensity: f32,             // 0.0-1.0, scales between base and target
    pub custom: Option<CustomVisuals>, // Override any visual parameter
    pub transition_ms: Option<u64>, // Transition duration (default 2500ms)
}

pub struct CustomVisuals {
    pub body_color: Option<[u8; 3]>,
    pub head_color: Option<[u8; 3]>,
    pub speed_multiplier: Option<f32>,
    pub emojis: Option<String>,     // Emoji chars as a string
    pub emoji_density: Option<f32>,
}
```

**Design philosophy:**
- MoodDirector lives on App, not inside Rain (clean separation)
- Rain receives simple override values; MoodDirector handles all interpolation
- Transitions happen at natural lifecycle boundaries (column resets), not every tick
- Agent can use presets for convenience or go fully custom for creative expression

---

### chat/ — Chat Message State & Rendering

**Files:** `src/chat/mod.rs` (81 lines), `src/chat/widget.rs` (143 lines)

**ChatState:**
- `messages: Vec<ChatMessage>` — Role (User/Assistant/System) + content
- `streaming: Option<String>` — Partial content during streaming responses
- `scroll_offset: usize` — Scroll from bottom (0 = pinned to latest)

**ChatWidget:**
- Renders as transparent overlay on left 45% of screen (min 30 cols)
- Messages grow bottom-up: most recent pinned to bottom
- Roles color-coded: User = white, Assistant = green, System = gray
- Streaming responses show with `...` indicator
- Word wrapping with whitespace-aware line breaking
- Renders directly into ratatui buffer cells (not using Paragraph widget) for transparency over rain

---

### input/ — Text Input State & Rendering

**Files:** `src/input/mod.rs` (84 lines), `src/input/widget.rs` (83 lines)

**InputState:**
- UTF-8 aware text buffer with character-level cursor
- `byte_offset()` converts char cursor to byte position for safe string operations
- Full editing: insert, backspace, delete, home, end, left, right

**InputWidget:**
- 3-row bordered box at bottom-left (matches chat width)
- Green border when focused, gray when not
- Horizontal scrolling when text exceeds width
- Green block cursor at current position

---

### effects/ — Visual Effects

**Files:** `src/effects/mod.rs` (214 lines) + 3 effect modules

Triggered on message send (centered on screen).

**Three effect types (cycled in order):**

| Effect | Duration | Description |
|--------|----------|-------------|
| Burst | 400ms | 8 particles radiate outward from center, green-tinted |
| Dissolve | 600ms | 12 hash/dot characters scatter randomly |
| Glitch | 300ms | 3-5 horizontal line displacements with random chars |

**EffectManager:**
- Maintains `Vec<Effect>` of active effects
- `tick()` removes expired effects
- Xorshift PRNG for deterministic (but varied) randomness in rendering

**Sub-modules (burst.rs, dissolve.rs, glitch.rs):**
These are legacy effect structs from an earlier architecture iteration. They define per-column speed/brightness boosts and particle systems but are not used by the current `EffectManager` — the current implementation renders effects inline in `effects/mod.rs`.

---

### gateway/ — WebSocket Gateway Client

**Files:** `src/gateway/mod.rs` (173 lines), `src/gateway/protocol.rs` (144 lines), `src/gateway/config.rs` (45 lines), `src/gateway/device.rs` (87 lines)

**Architecture:**
- Spawned as an isolated tokio task via `spawn_gateway(config)`
- Communicates with App via mpsc channels:
  - `GatewayCommand` (App → Gateway): `SendMessage(String)`, `Disconnect`
  - `GatewayAction` (Gateway → App): `Connected`, `Disconnected`, `ChatDelta`, `ChatComplete`, `Error`, `MoodUpdate`

**Connection lifecycle:**
1. Connect to WebSocket URL
2. Receive `auth.challenge` → sign with Ed25519 device key → send `auth.respond`
3. Receive `auth.hello` → mark as connected
4. Exchange `chat.send` / `chat.delta` / `chat.complete` / `mood.update`
5. On disconnect: exponential backoff (1s → 30s max), auto-reconnect

**Protocol (JSON-RPC v3):**

Outgoing (client → server):
```json
{"jsonrpc":"3.0","id":1,"method":"auth.respond","params":{"device_id":"matrix-abc123","signature":"base64..."}}
{"jsonrpc":"3.0","id":2,"method":"chat.send","params":{"content":"Hello!"}}
```

Incoming (server → client):
```json
{"method":"auth.challenge","params":{"challenge":"random-string"}}
{"method":"auth.hello"}
{"method":"chat.delta","params":{"delta":"streaming token"}}
{"method":"chat.complete","params":{"content":"full response"}}
{"method":"mood.update","params":{"mood":"excited","intensity":0.8,"transition_ms":3000}}
```

**Device identity (device.rs):**
- Ed25519 keypair stored at `~/.openclaw/identity/device-matrix.json`
- Auto-generated on first run
- Device ID format: `matrix-{first 16 hex chars of public key}`

**Config resolution (config.rs):**
- Priority: CLI args → env vars (`OPENCLAW_GATEWAY_URL`, `OPENCLAW_TOKEN`) → `~/.openclaw/openclaw.json`

---

### settings/ — In-App Settings Menu

**Files:** `src/settings/mod.rs` (167 lines), `src/settings/widget.rs` (96 lines)

Full-screen settings overlay that replaces the rain. Accessible via `Ctrl+S`.

**7 configurable parameters:**

| Setting | Options |
|---------|---------|
| Color | green, red, blue, white, RGB tuples |
| Head | white, green, red, blue, hex codes |
| Group | bin, jap, classic, num, alphalow, alphaup, arrow, cards, clock, crab, earth, emojis, moon, shapes, smile, plants, opensource, pglangs |
| Direction | south, north, west, east |
| Speed | Various min,max pairs |
| Shade | off, on |
| Gradient | Various hex shade targets |

**UX:** Navigate with Up/Down, change values with Left/Right. On close (Esc or Ctrl+S): applies settings, persists to disk, rebuilds rain engine from scratch, updates mood baseline.

---

### persist.rs — Settings Persistence

**File:** `src/persist.rs` (91 lines)

Saves/loads user settings to `~/.config/openclaw-mood/settings.toml` (via `dirs` crate).

**Persisted fields:** color, head, group, direction, speed, shade, shade_gradient, bg_color.

Settings are applied at startup after CLI args (so explicit CLI args override saved preferences).

---

### cli.rs — CLI Argument Parsing

**File:** `src/cli.rs` (472 lines)

Comprehensive CLI via clap with derive macros.

**Key flags:**

| Flag | Description | Default |
|------|-------------|---------|
| `-s, --shade` | Enable gradient shading on trails | false |
| `-g, --group` | Character set | bin |
| `-C, --color` | Rain body color | green |
| `-B, --bg-color` | Background color | none (terminal default) |
| `-G, --shade-gradient` | Shade target color | #000000 |
| `-H, --head` | Head character color | white |
| `-d, --direction` | Rain direction | south |
| `-S, --speed` | Speed range (min,max ms) | 0,200 |
| `--gateway-url` | WebSocket URL | env/config |
| `--token` | Auth token | env/config |
| `--offline` | Screensaver mode | false |

**Custom character groups:**
- `Grouping` enum wraps ezemoji `CharGroup` or custom `Group` (Unicode ranges)
- Special groups: "classic" (katakana + symbols), "opensource" (nerd font icons), "pglangs" (programming language icons)
- Color parsing: named colors ("green", "red", etc.) or RGB tuples ("0,255,0") or hex ("#00FF00")

---

### config.rs & theme.rs — Legacy Configuration

**Files:** `src/config.rs` (128 lines), `src/theme.rs` (53 lines)

These are from an earlier architecture iteration. They define a separate `Config` struct (with clap), `Speed`/`Density`/`Charset` enums, and a `Theme` struct. They are compiled but **not used by the main application path** — the active configuration flows through `cli.rs` instead. They are referenced by `rain/characters.rs` and `rain/column.rs` which are also legacy modules.

---

## Data Flow

### Message Send Flow
```
User types message → InputState.insert_char()
User presses Enter → InputState.take_text()
                   → ChatState.push_user_message()
                   → EffectManager.trigger() (visual burst)
                   → GatewayCommand::SendMessage via mpsc
                   → Gateway task sends JSON-RPC "chat.send"
                   → Server streams back "chat.delta" tokens
                   → GatewayAction::ChatDelta via mpsc
                   → ChatState.append_streaming()
                   → Server sends "chat.complete"
                   → GatewayAction::ChatComplete via mpsc
                   → ChatState.finish_streaming()
```

### Mood Update Flow
```
Server sends JSON-RPC "mood.update"
→ IncomingFrame::MoodUpdate (protocol.rs parses params)
→ GatewayAction::MoodUpdate via mpsc
→ App.process_gateway_actions()
→ MoodDirector.apply_mood()
  → Resolve preset visuals (if mood specified)
  → Apply custom overrides
  → Factor in intensity (lerp between base and target)
  → Retarget body/head/speed tweens (snapshot current as new start)
  → Set emoji accent target (pool + density + transition speed)
→ Next tick: App.tick()
  → MoodDirector.tick() — advance emoji transitions
  → Push body_color/head_color to Rain.set_override_colors()
  → Push emoji pool/density to Rain.set_emoji_accents()
  → Rain.update() — ready columns enter queue
  → Rain.update_screen_buffer() — queued columns:
    → On reset: adopt override colors, roll for emoji head
    → Head cell: use emoji_heads[col] if set, else normal char
    → Body cells: use override-derived shade gradient
```

---

## Gateway Protocol

### Authentication Handshake
```
Client connects via WebSocket
  ←  {"method":"auth.challenge","params":{"challenge":"random-nonce"}}
  →  {"jsonrpc":"3.0","id":1,"method":"auth.respond","params":{
        "device_id":"matrix-abc123def456",
        "signature":"base64-ed25519-signature-of-challenge"
      }}
  ←  {"method":"auth.hello"}
```

### Chat Messages
```
  →  {"jsonrpc":"3.0","id":2,"method":"chat.send","params":{"content":"What is Rust?"}}
  ←  {"method":"chat.delta","params":{"delta":"Rust is "}}
  ←  {"method":"chat.delta","params":{"delta":"a systems "}}
  ←  {"method":"chat.delta","params":{"delta":"programming language."}}
  ←  {"method":"chat.complete","params":{"content":"Rust is a systems programming language."}}
```

### Mood Updates
```
  ←  {"method":"mood.update","params":{
        "mood":"excited",
        "intensity":0.8,
        "transition_ms":3000
      }}

  ←  {"method":"mood.update","params":{
        "mood":null,
        "intensity":1.0,
        "custom":{
          "body_color":[255,100,50],
          "head_color":[255,255,200],
          "emojis":"🎨🖌️✨",
          "emoji_density":0.15,
          "speed_multiplier":0.7
        },
        "transition_ms":5000
      }}
```

---

## Mood System Deep Dive

### Transition Model

All visual changes are **retargetable tweens**. If a new mood arrives while a transition is in progress, the current interpolated value becomes the new starting point — no jumps or discontinuities.

```
Time →
        Mood: Neutral          Mood: Excited           Mood: Serene
Color:  ████████████           ████████████            ████████████
        green ─────smooth─────▶ magenta ───smooth────▶ teal
                    ↑                        ↑
              2.5s transition          2.5s transition
```

If "Serene" arrives at 50% through the Neutral→Excited transition:
```
Color:  green ──────▶ midpoint(green,magenta) ──────▶ teal
                      ↑ snapshot as new "from"
```

### Oklab Color Space

Colors are interpolated in the [Oklab](https://bottosson.github.io/posts/oklab/) perceptual color space rather than RGB. This produces:
- No muddy browns when transitioning between complementary colors
- Perceptually uniform brightness throughout the transition
- Visually smooth gradients that look intentional

### Emoji Accent Strategy

Emojis are scattered sparsely (5-12% of columns) as head characters on rain drops. They are assigned at natural lifecycle boundaries (when a column resets after its drop falls off screen), not every tick. This prevents:
- Flickering (constantly reassigning heads)
- Unnatural sudden appearance of all emojis
- Performance overhead of per-cell decisions

During mood transitions, both the old and new emoji pools are merged, and density interpolates linearly. New columns draw from the merged pool until the transition completes.

### Easing Function

All tweens use ease-in-out-cubic for natural-feeling motion:
```
f(t) = { 4t³                    if t < 0.5
        { 1 - (-2t + 2)³ / 2    if t >= 0.5
```

---

## Rendering Pipeline

Each frame (every 50ms):

```
1. App.tick()
   ├── MoodDirector.tick()              — advance emoji blend
   ├── Rain.set_override_colors(body, head) — push mood colors
   ├── Rain.set_emoji_accents(pool, density) — push emoji state
   ├── Rain.update()                     — check timing, queue columns
   ├── Rain.update_screen_buffer()       — update Cell array
   └── EffectManager.tick()              — expire old effects

2. App.draw(frame)
   ├── [Settings mode] → SettingsWidget (full screen, return)
   ├── RainWidget.render()               — map screen_buffer to ratatui Buffer
   │   ├── Fill background color
   │   ├── For each cell: set char, fg color (truecolor or 256-color)
   │   └── Handle double-width characters
   ├── ChatWidget.render()               — overlay messages (left 45%)
   │   ├── Word wrap all messages
   │   ├── Bottom-up layout with scroll offset
   │   └── Direct cell writes for transparency
   ├── EffectsWidget.render()            — burst/dissolve/glitch overlays
   ├── InputWidget.render()              — bordered input box (typing mode)
   └── draw_status_bar()                 — mode + mood + connection + hints
```

---

## Keybindings

### Viewing Mode
| Key | Action |
|-----|--------|
| `q` / `Q` / `Esc` | Quit |
| `Ctrl+C` | Quit |
| `i` / `/` | Enter typing mode |
| `m` | Cycle through mood presets (debug) |
| `Ctrl+S` | Open settings |
| `Up` / `Down` | Scroll chat |

### Typing Mode
| Key | Action |
|-----|--------|
| `Esc` | Back to viewing mode |
| `Ctrl+C` | Quit |
| `Enter` | Send message |
| `Backspace` | Delete char before cursor |
| `Delete` | Delete char at cursor |
| `Left` / `Right` | Move cursor |
| `Home` / `End` | Jump to start/end |
| `Up` / `Down` | Scroll chat |
| Any char | Insert at cursor |

### Settings Mode
| Key | Action |
|-----|--------|
| `Esc` / `Ctrl+S` | Apply, save, and close |
| `Ctrl+C` | Quit |
| `Up` / `Down` | Navigate settings |
| `Left` / `Right` | Change value |

---

## Configuration

### File Locations

| File | Purpose |
|------|---------|
| `~/.config/rusty-rain/config.toml` | Rain engine config (character groups, colors) |
| `~/.config/openclaw-mood/settings.toml` | Persisted UI settings (auto-saved) |
| `~/.openclaw/openclaw.json` | Gateway URL and token |
| `~/.openclaw/identity/device-matrix.json` | Ed25519 device keypair |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENCLAW_GATEWAY_URL` | WebSocket gateway URL |
| `OPENCLAW_TOKEN` | Authentication token |
| `COLORTERM` | Terminal color support (`truecolor` or `24bit` for RGB) |

---

## Testing

```bash
# Run all tests
cargo test

# Run non-snapshot tests only (snapshot tests are timing-dependent)
cargo test -- --skip test_screen_buffer --skip test_large_letters

# Run mood system tests
cargo test mood
```

**Test coverage:**
- 8 unit tests for mood system (tweens, Oklab, presets, custom visuals, speed clamping, easing)
- 1 unit test for shade gradient generation
- 4 snapshot tests for rain rendering (timing-dependent, may be flaky due to `Instant::now()`)

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` 0.29 | TUI framework (widgets, buffer, layout) |
| `crossterm` 0.29 | Terminal I/O, keyboard events |
| `tokio` 1.x (full) | Async runtime, timers, channels |
| `tokio-tungstenite` 0.26 | WebSocket client |
| `clap` 4.5 | CLI argument parsing |
| `serde` / `serde_json` | JSON serialization for protocol |
| `toml` 0.9 | Config file parsing |
| `rand` 0.10 | Random number generation |
| `ezemoji` 2.0 | Unicode character group definitions |
| `ed25519-dalek` 2.x | Ed25519 signing for device auth |
| `base64` 0.22 | Base64 encoding for signatures |
| `hex` 0.4 | Hex encoding for device IDs |
| `dirs` 6.x | Cross-platform config directory resolution |
| `futures` 0.3 | Stream/Sink traits for WebSocket |
| `insta` 1.43 (dev) | Snapshot testing |
| `pretty_assertions` 1.4 (dev) | Better test diff output |
