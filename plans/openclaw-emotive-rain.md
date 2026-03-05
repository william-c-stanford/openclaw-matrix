# feat: Ship Emotive Rain as Installable OpenClaw Tool

## Overview

Make `openclaw-matrix` a dead-simple installable Rust binary that any OpenClaw user can run with one command. OpenClaw (the agent) learns the mood protocol from its own instructions and emits `<mood>` tags at every emotional inflection point — costing ~3-5 tokens per tag — to drive real-time visual changes in the matrix rain TUI.

**Deliverables:**

1. **One-command install** — `cargo install openclaw-matrix` or `curl | sh` via prebuilt binaries
2. **OpenClaw agent instructions** — CLAUDE.md mood protocol so OpenClaw knows how to emit `<mood>` tags
3. **Gateway mood extraction** — middleware that strips `<mood>` tags from agent output and relays as `mood.update` WebSocket frames
4. **TUI polish** — mood settings, speed multiplier fix, config parser fix, demo mode

**Not in scope:** Claude Code skills, MCP bridges, npm packages. This is OpenClaw-only.

---

## Problem Statement

The emotive rain engine is fully built on the TUI side — 8 mood presets, Oklab color interpolation, retargetable tweens, emoji accents, 587 lines of production code with 7 unit tests. But it's trapped behind a debug `m` key:

- No install path exists (must clone and `cargo run`)
- No agent instructions exist (OpenClaw doesn't know the mood protocol)
- No gateway middleware extracts `<mood>` tags from agent output
- No user control over mood frequency
- Speed multiplier is computed but never applied to rain
- Config parser doesn't match `openclaw.json`'s actual schema

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│  OpenClaw Agent                                  │
│                                                  │
│  CLAUDE.md includes mood protocol:               │
│  "Append <mood preset='curious'/> at turns"      │
│                                                  │
│  Response: "Great question!<mood:curious>"        │
│            ~~~~~~~~~~~~~~~  ~~~~~~~~~~~~~~        │
│            displayed text   stripped by gateway   │
└──────────────────┬──────────────────────────────-┘
                   │ agent output stream
                   ▼
┌──────────────────────────────────────────────────┐
│  OpenClaw Gateway (ws://localhost:18789/ws)       │
│                                                  │
│  1. Accumulate streaming tokens in buffer        │
│  2. Detect <mood .../> pattern                   │
│  3. Strip tag → send chat.delta (clean text)     │
│  4. Parse attributes → send mood.update frame    │
│  5. Throttle per TUI's frequency preference      │
└──────────────────┬──────────────────────────────-┘
                   │ WebSocket JSON-RPC
                   ▼
┌──────────────────────────────────────────────────┐
│  openclaw-matrix TUI                             │
│                                                  │
│  mood.update → MoodDirector → Rain visuals       │
│  Colors, speed, emoji accents transition smoothly│
│  User settings: Mood off/rare/normal/expressive  │
└──────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Fix Config Parser & Zero-Config Connection

The TUI's `gateway/config.rs` currently expects a flat `{ gateway_url, token }` struct, but `~/.openclaw/openclaw.json` has a nested schema. This silently breaks auto-discovery — users run the binary and get offline mode even when the gateway is running.

**The actual `openclaw.json` schema:**

```json
{
  "gateway": {
    "port": 18789,
    "mode": "local",
    "bind": "loopback",
    "auth": {
      "mode": "token",
      "token": "57b2496a..."
    }
  }
}
```

**Changes:**

Update `src/gateway/config.rs` to deserialize the nested structure:

```rust
#[derive(Deserialize)]
struct OpenClawConfig {
    gateway: Option<GatewaySection>,
}

#[derive(Deserialize)]
struct GatewaySection {
    port: Option<u16>,
    bind: Option<String>,   // "loopback" → 127.0.0.1
    auth: Option<AuthSection>,
}

#[derive(Deserialize)]
struct AuthSection {
    token: Option<String>,
}
```

Construct URL as `ws://127.0.0.1:{port}/ws` when `bind == "loopback"`.

**Files to modify:**
- `src/gateway/config.rs` — new nested deserialization

**Acceptance criteria:**
- [ ] `openclaw-matrix` (no args) auto-connects to gateway when `~/.openclaw/openclaw.json` exists
- [ ] Falls back gracefully to offline mode when config is missing or gateway unreachable
- [ ] CLI `--gateway-url` flag still overrides config file
- [ ] Malformed config file logs warning to stderr, doesn't crash

---

### Phase 2: OpenClaw Agent Instructions (CLAUDE.md)

Create a mood protocol section in OpenClaw's instruction set. This is what teaches the agent to emit `<mood>` tags. It costs ~150 tokens in the system prompt and ~3-5 tokens per mood emission.

**File to create:** `~/.openclaw/CLAUDE.md` mood protocol section (or append to existing)

**The mood protocol instructions:**

```markdown
## Emotive Rain Protocol

You are connected to a Matrix rain TUI that visualizes your emotional state.
Append a `<mood>` tag at emotional inflection points. The gateway strips it
before the user sees your text.

### Format

<mood:preset>             — basic mood
<mood:preset:intensity>   — with intensity (0.0-1.0, default 0.8)

### Presets

| Preset | Color | Speed | Emojis | When |
|--------|-------|-------|--------|------|
| curious | Blue | Slower | ? 🔍 🤔 | Exploring, questioning |
| excited | Magenta | Faster | ✨ 🔥 ⚡ | Breakthroughs, enthusiasm |
| contemplative | Indigo | Slowest | 💭 🌌 | Deep thinking |
| frustrated | Red-orange | Faster | 💢 ⚠️ | Debugging, walls |
| amused | Gold | Normal | 😂 😄 | Humor, playfulness |
| focused | Silver | Fast | 🎯 ⚙️ | Heads-down coding |
| serene | Teal | Slowest | 🌿 🌻 | Calm, wrapping up |

### Rules

1. NEVER mention or narrate your mood. The visual IS the message.
2. Most messages: no tag. Silence is eloquent. Neutral is default.
3. Shift at emotional inflection points — roughly every 3-5 messages.
4. Intensity 0.3-0.5 for subtle, 0.7-1.0 for strong.
5. After intense moments, return: <mood:neutral:0>
```

**Design decisions:**

- **Colon-delimited format** (`<mood:curious:0.8>`) instead of XML attributes — saves 3-5 tokens vs `<mood preset="curious" intensity="0.8"/>`. The gateway regex handles both.
- **No custom colors in agent instructions** — keep the agent to preset names only. Custom colors are a gateway/debug concern, not something the agent should decide (prevents hallucinated color values).
- **No `transition_ms` in agent instructions** — the TUI default of 2500ms is always appropriate. Exposing this adds tokens for zero user benefit.

**Acceptance criteria:**
- [ ] OpenClaw agent emits `<mood:preset>` tags at emotional transitions
- [ ] Tags are ~3-5 tokens overhead (verify with tokenizer)
- [ ] Agent never narrates mood changes
- [ ] Agent defaults to no tag (most messages)

---

### Phase 3: Gateway Mood Extraction Middleware

The gateway server must strip `<mood>` tags from agent output and convert them to `mood.update` WebSocket frames. This is the **critical path** — without it, the entire system is dead.

**Tag format (gateway must parse both):**

```
# Compact (from agent instructions)
<mood:curious>
<mood:excited:0.8>
<mood:neutral:0>

# Verbose (from legacy/manual use)
<mood preset="curious"/>
<mood preset="excited" intensity="0.8"/>
```

**Regex for both formats:**

```
<mood(?::(\w+)(?::([0-9.]+))?|[^>]*?)\/?>
```

**Parsing rules:**

1. Accumulate streaming tokens in a buffer
2. On detecting `<mood`, buffer until `>` is found (max 200 chars — force flush if exceeded)
3. Strip matched tag from text, emit buffered clean text as `chat.delta`
4. Parse mood name → validate against known preset list → emit `mood.update` frame
5. If mood name is unknown, strip tag silently (don't relay as `mood.update`), warn in server log

**Attribute → JSON-RPC mapping:**

| Tag Format | JSON-RPC `mood.update` params |
|------------|-------------------------------|
| `<mood:curious>` | `{"mood":"curious","intensity":1.0}` |
| `<mood:excited:0.8>` | `{"mood":"excited","intensity":0.8}` |
| `<mood:neutral:0>` | `{"mood":"neutral","intensity":0.0}` |
| `<mood preset="focused" intensity="0.7"/>` | `{"mood":"focused","intensity":0.7}` |

**Streaming edge cases:**

| Scenario | Behavior |
|----------|----------|
| Tag split across chunks: `<mood:cu` + `rious>` | Buffer until `>`, then parse |
| Multiple tags in one response | Last tag wins (only one `mood.update` per response) |
| Malformed: `<mood:nonexistent>` | Strip from text, do NOT emit `mood.update`, log warning |
| Partial `<` at end of chunk | Buffer the `<`, check next chunk |
| Buffer exceeds 200 chars without closing `>` | Flush buffer as-is (not a mood tag) |
| No tag in response | Pass through unchanged, no `mood.update` |

**Files to create/modify:** In the gateway server codebase (not in `openclaw-matrix` repo)

**Acceptance criteria:**
- [ ] `<mood:preset>` tags stripped from chat text before TUI sees them
- [ ] Parsed moods sent as JSON-RPC `mood.update` notifications over WebSocket
- [ ] Both compact (`<mood:curious>`) and verbose (`<mood preset="curious"/>`) formats work
- [ ] Streaming mode handles split tags correctly
- [ ] Malformed/unknown moods stripped but not relayed
- [ ] Multiple tags per response: last wins

---

### Phase 4: TUI Enhancements

#### 4a. Add Mood Frequency Setting

Add an 8th entry to the settings panel with client-side throttle enforcement.

**Changes to `src/settings/mod.rs`:**

```rust
SettingEntry {
    label: "Mood",
    options: vec!["off".into(), "rare".into(), "normal".into(), "expressive".into()],
    selected: 2, // default: normal
}
```

**Changes to `src/persist.rs`:**

Add `mood_frequency: Option<String>` to `Saved` struct. Default to `"normal"`.

**Changes to `src/app.rs`:**

Track `last_mood_applied: Option<Instant>` on `App`. In `process_gateway_actions()`, before applying a `MoodUpdate`:

```rust
let min_interval = match mood_frequency.as_str() {
    "off" => return,  // drop all mood updates
    "rare" => Duration::from_secs(30),
    "normal" => Duration::from_secs(8),
    "expressive" => Duration::from_secs(2),
    _ => Duration::from_secs(8),
};

if let Some(last) = self.last_mood_applied {
    if last.elapsed() < min_interval {
        return;  // throttled
    }
}
self.last_mood_applied = Some(Instant::now());
```

When user sets mood to "off" while a mood is active, trigger a synthetic reset:

```rust
self.mood_director.apply_mood(&MoodUpdate {
    mood: None,
    intensity: 0.0,
    custom: None,
    transition_ms: Some(2500),
});
```

**Files to modify:**
- `src/settings/mod.rs` — add Mood entry
- `src/persist.rs` — add `mood_frequency` field
- `src/app.rs` — throttle logic + "off" reset

**Acceptance criteria:**
- [ ] Settings panel shows Mood: off/rare/normal/expressive
- [ ] Mood frequency persists across restarts
- [ ] Throttle drops rapid mood updates per frequency table
- [ ] Setting mood to "off" smoothly transitions back to baseline

---

#### 4b. Apply Speed Multiplier to Rain

The `MoodDirector` computes `speed_multiplier()` but it's never applied. This means "excited = faster" and "contemplative = slower" have no effect — half the mood expressiveness is missing.

**Changes to `src/rain/mod.rs`:**

Add a `speed_multiplier: f32` field to `Rain` and a `set_speed_multiplier(f32)` method that scales per-column tick durations.

**Changes to `src/app.rs` `tick()`:**

```rust
// After applying override colors:
self.rain.set_speed_multiplier(self.mood_director.speed_multiplier());
```

**Files to modify:**
- `src/rain/mod.rs` — add speed multiplier field + setter
- `src/app.rs` — apply speed in tick()

**Acceptance criteria:**
- [ ] Excited mood visibly speeds up rain
- [ ] Contemplative/serene mood visibly slows rain
- [ ] Speed transitions smoothly (not instant jumps)
- [ ] Speed returns to normal when mood resets to neutral

---

#### 4c. Fix Baseline Recomputation

`MoodDirector::update_base()` stores new base colors but doesn't recompute the active mood's tween targets. Changing base color during an active mood produces wrong final colors.

**Fix in `src/mood.rs`:**

```rust
pub fn update_base(&mut self, body: [u8; 3], head: [u8; 3]) {
    self.base_body = body;
    self.base_head = head;
    // Recompute active mood against new base
    if let Some(mood) = self.current_mood {
        self.apply_mood(&MoodUpdate {
            mood: Some(mood),
            intensity: self.intensity,
            custom: None,
            transition_ms: Some(500), // fast transition for settings change
        });
    }
}
```

**Files to modify:**
- `src/mood.rs` — fix `update_base()`

---

#### 4d. Add Malformed Mood Update Logging

Failed `mood.update` parsing is silently swallowed in `protocol.rs`. Add stderr logging.

**Fix in `src/gateway/protocol.rs`:**

```rust
"mood.update" => {
    if let Some(params) = frame.params {
        match serde_json::from_value::<MoodUpdate>(params.clone()) {
            Ok(update) => return IncomingFrame::MoodUpdate(update),
            Err(e) => {
                eprintln!("[mood] failed to parse mood.update: {e} — raw: {params}");
            }
        }
    }
    return IncomingFrame::Unknown(text.to_string());
}
```

**Files to modify:**
- `src/gateway/protocol.rs` — add logging on parse failure

---

#### 4e. Add `--demo` Flag

Let users experience mood effects without a gateway. Auto-cycles through all 8 presets on a timer.

**Changes to `src/cli.rs`:**

```rust
#[arg(long, help = "Demo mode: auto-cycle through all mood presets")]
pub demo: bool,
```

**Changes to `src/app.rs`:**

In the tick loop, when `demo` mode is active:

```rust
const DEMO_MOODS: &[Mood] = &[
    Mood::Curious, Mood::Excited, Mood::Contemplative,
    Mood::Frustrated, Mood::Amused, Mood::Focused, Mood::Serene,
];
const DEMO_INTERVAL: Duration = Duration::from_secs(8);

if self.demo_mode {
    if self.demo_last_change.elapsed() >= DEMO_INTERVAL {
        self.demo_index = (self.demo_index + 1) % DEMO_MOODS.len();
        self.mood_director.apply_mood(&MoodUpdate {
            mood: Some(DEMO_MOODS[self.demo_index]),
            intensity: 0.8,
            custom: None,
            transition_ms: Some(2500),
        });
        self.demo_last_change = Instant::now();
    }
}
```

**Files to modify:**
- `src/cli.rs` — add `--demo` flag
- `src/app.rs` — demo cycle logic

**Acceptance criteria:**
- [ ] `openclaw-matrix --demo` cycles through all moods every 8 seconds
- [ ] Status bar shows current demo mood name
- [ ] Demo mode works fully offline (no gateway needed)
- [ ] User can still interact (change settings, press keys) during demo

---

### Phase 5: Distribution via cargo-dist

Make installation a one-command experience for any platform, no Rust toolchain required.

**Step 1: Add crates.io metadata to `Cargo.toml`:**

```toml
[package]
name = "openclaw-matrix"
version = "0.1.0"
edition = "2024"
license = "MIT"
description = "Matrix rain TUI with agent-driven mood visualization for OpenClaw"
repository = "https://github.com/william-c-stanford/openclaw-matrix"
keywords = ["matrix", "rain", "tui", "terminal", "openclaw"]
categories = ["command-line-utilities"]
```

**Step 2: Initialize cargo-dist:**

```bash
cargo install cargo-dist --locked
dist init --yes
```

This generates:
- `.github/workflows/release.yml` — CI pipeline for building cross-platform binaries
- `Cargo.toml` additions — dist profile and installer config

**Step 3: Configure target platforms and installers:**

```toml
[workspace.metadata.dist]
cargo-dist-version = "0.27.0"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
targets = [
    "aarch64-apple-darwin",     # macOS Apple Silicon
    "x86_64-apple-darwin",      # macOS Intel
    "x86_64-unknown-linux-gnu", # Linux x86_64
]
```

**Step 4: Release workflow:**

```bash
# Bump version in Cargo.toml, then:
git tag v0.1.0
git push --tags
# CI builds binaries, creates GitHub Release, publishes installers
```

**User install commands (what end users run):**

```bash
# macOS/Linux — shell installer (no Rust needed)
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/william-c-stanford/openclaw-matrix/releases/latest/download/openclaw-matrix-installer.sh | sh

# Homebrew
brew install william-c-stanford/openclaw/openclaw-matrix

# Cargo (for Rust devs)
cargo install openclaw-matrix
```

**Files to create/modify:**
- `Cargo.toml` — add metadata + dist config
- `.github/workflows/release.yml` — generated by `dist init`

**Acceptance criteria:**
- [ ] `cargo install openclaw-matrix` works from crates.io
- [ ] Shell installer downloads prebuilt binary on macOS/Linux
- [ ] Binary runs immediately with zero configuration
- [ ] `openclaw-matrix --demo` showcases mood effects without gateway
- [ ] `openclaw-matrix` (no args) auto-connects when `~/.openclaw/openclaw.json` exists

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Install-to-first-rain time | < 30 seconds (prebuilt binary) |
| Install-to-first-mood-shift | < 5 minutes (with running gateway) |
| Agent mood tag overhead | < 5 tokens per emission |
| System prompt cost | < 200 tokens for mood protocol |
| Mood matches conversation tone | > 80% of shifts feel natural |
| User reports mood as "distracting" | < 10% |

---

## Dependencies & Prerequisites

| Dependency | Status | Blocker? |
|-----------|--------|----------|
| TUI mood rendering engine | **Complete** | No |
| TUI gateway protocol (mood.update parsing) | **Complete** | No |
| Gateway server (running, WebSocket) | **Exists** | No |
| Gateway mood tag extraction middleware | **Not built** | **YES** |
| OpenClaw agent instruction mechanism | **Exists** (CLAUDE.md) | No |
| crates.io account | Needed | No |
| GitHub Actions for cargo-dist | Needed | No |

**Critical blocker:** Phase 3 (gateway mood extraction) must be built for the live pipeline to work. Phases 1, 2, 4, and 5 can proceed independently. Phase 4e (`--demo` flag) is the fallback that lets users experience moods without the gateway.

---

## Implementation Order

```
Phase 1 (config fix)     ──→ Phase 5 (distribution)
                              ↓
Phase 2 (agent instructions) ─┤  Can ship independently
                              ↓
Phase 4a-e (TUI polish)  ────┤  Can ship independently
                              ↓
Phase 3 (gateway middleware) ─→ Full pipeline live
```

**Recommended approach:**
1. Fix config parser + add demo mode + add speed multiplier (high-impact TUI improvements)
2. Ship v0.1.0 with `cargo-dist` (users can install and see demo mode)
3. Add CLAUDE.md mood protocol for OpenClaw agent
4. Build gateway middleware (unblocks the full live pipeline)
5. Add mood frequency setting + bug fixes
6. Ship v0.2.0 with full mood support

---

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Agent over-emits mood tags | Medium | Distracting | Frequency guidelines + client-side throttle |
| Agent narrates mood | Low | Breaks immersion | Explicit "NEVER narrate" rule |
| Gateway middleware delays text delivery | Medium | UX degradation | 200-char buffer cap, <50ms parsing budget |
| Config parser change breaks existing setups | Low | Can't connect | Support both flat and nested schemas |
| Demo mode overshadows live moods | Low | Reduced gateway adoption | Demo mode clearly labeled in status bar |

---

## References

### Internal
- `openclaw-matrix/src/mood.rs` — MoodDirector, tweens, presets, Oklab (587 lines)
- `openclaw-matrix/src/gateway/protocol.rs` — mood.update parsing
- `openclaw-matrix/src/gateway/config.rs` — config resolution (needs fix)
- `openclaw-matrix/src/app.rs:100-125` — mood → rain integration in tick()
- `openclaw-matrix/src/settings/mod.rs` — settings panel (needs Mood entry)
- `openclaw-matrix/src/persist.rs` — TOML persistence (needs mood_frequency)
- `openclaw-matrix/ARCHITECTURE.md` — full system documentation

### External
- [cargo-dist Quickstart](https://axodotdev.github.io/cargo-dist/book/quickstart/rust.html)
- [Packaging Rust CLI Tools](https://rust-cli.github.io/book/tutorial/packaging.html)
- [Automated Rust Releases](https://blog.orhun.dev/automated-rust-releases/)
