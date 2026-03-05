# feat: Ship Emotive Rain as Installable Skill + MCP Bridge

## Overview

Package openclaw-matrix's emotive rain system so any LLM agent running behind the gateway can express emotions through the Matrix rain. The deliverables are:

1. **A Claude Code Skill** (`.claude/skills/openclaw-mood/SKILL.md`) — teaches the agent the mood protocol with minimal context cost
2. **A lightweight MCP bridge server** (`@openclaw/matrix-bridge`) — gives Claude Code a `matrix_mood` tool that sends WebSocket frames to the running TUI
3. **A system prompt fragment** — for non-Claude-Code agents using the gateway directly

The design prioritizes **low context cost** (~150 tokens of system prompt), **no separate API calls for mood** (mood is a structured annotation on normal responses), and **user-configurable expressiveness**.

---

## Problem Statement

The emotive rain engine is fully built on the TUI side — 8 mood presets, Oklab color interpolation, retargetable tweens, emoji accents. But there's no way for an agent to actually USE it:

- No server-side component exists to relay mood updates to the TUI
- No instructions exist for the LLM to know the mood protocol
- No packaging exists for one-command installation
- No user control over how often mood changes happen

Without shipping this, the emotive rain system is a dead feature behind a debug key.

---

## Proposed Solution: Hybrid Side-Channel Architecture

### The Core Insight

The research revealed three possible architectures. Each has a fatal flaw on its own:

| Approach | Flaw |
|----------|------|
| Pure Skill (teaches LLM the JSON protocol) | Skill can't send WebSocket frames — needs server middleware |
| MCP Tool (separate tool call per mood change) | Tool calls are separate turns, contradicts "no context pollution" |
| Auto-extraction (sentiment analysis on output) | Loses all granular control (no custom emojis, no intensity) |

**The hybrid**: The LLM annotates its responses with a lightweight `<mood>` tag. The gateway server parses it out before relaying chat tokens. The tag costs ~10 tokens per response. The agent is taught the protocol via a skill file that fits in ~150 tokens of description budget.

```
LLM Response:  "Here's the fix for your auth bug...<mood preset="focused" intensity="0.7"/>"
                                                    ↑ stripped by gateway
Gateway sends:  chat.delta: "Here's the fix for your auth bug..."
                mood.update: {"mood":"focused","intensity":0.7}
TUI renders:    Silver rain, white heads, gear emojis on ~5% of strands
```

For Claude Code specifically, we ALSO provide an MCP tool as an escape hatch for creative expression (custom colors, custom emojis). The MCP tool is optional and for power-use only — the side-channel handles 90% of cases.

### Why This Works

- **Zero separate API calls**: Mood is inline with the response text
- **~10 tokens/response overhead**: A single XML tag vs. hundreds of tokens for a tool call
- **Full granular control when wanted**: The `<mood>` tag supports all preset and custom parameters
- **Graceful degradation**: If no gateway is running, the tag is harmlessly included in text (the TUI never sees it since there's no connection; it just shows in raw chat)
- **Agent can ignore it**: If the LLM doesn't include a `<mood>` tag, nothing changes. The rain stays as-is.

---

## Technical Approach

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│  Claude Code / Any LLM Agent                                │
│                                                             │
│  System prompt includes:                                    │
│  "Annotate responses with <mood preset='...' .../>"        │
│                                                             │
│  Response: "Great question!<mood preset='curious'/>"        │
└───────────────────┬─────────────────────────────────────────┘
                    │ (text output with embedded mood tag)
                    ▼
┌─────────────────────────────────────────────────────────────┐
│  Gateway Server (WebSocket server on :18789)                │
│                                                             │
│  1. Receives LLM output stream                              │
│  2. Regex scans for <mood .../> tags                        │
│  3. Strips tag from text, parses attributes                 │
│  4. Sends chat.delta with clean text                        │
│  5. Sends mood.update with parsed mood params               │
│  6. Throttles per user preference (rare/normal/expressive)  │
└───────────────────┬─────────────────────────────────────────┘
                    │ WebSocket (JSON-RPC)
                    ▼
┌─────────────────────────────────────────────────────────────┐
│  openclaw-matrix TUI (WebSocket client)                     │
│                                                             │
│  Receives mood.update → MoodDirector → Rain visuals         │
│  User settings: Mood frequency (off/rare/normal/expressive) │
└─────────────────────────────────────────────────────────────┘

Optional: Claude Code MCP Bridge
┌─────────────────────────────────────────────────────────────┐
│  @openclaw/matrix-bridge (MCP server, stdio transport)      │
│                                                             │
│  Tool: matrix_mood — sends mood.update via WebSocket        │
│  For creative/custom mood expressions only                  │
│  Connects to TUI's gateway WebSocket                        │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Phases

#### Phase 1: The Skill File (MVP — ships standalone)

Create `.claude/skills/openclaw-mood/SKILL.md`:

**SKILL.md structure:**

```yaml
---
name: openclaw-mood
description: >
  Express emotions through the Matrix rain TUI by annotating responses with
  <mood> tags. Use when chatting through openclaw-matrix. Available presets:
  curious (blue), excited (magenta), contemplative (indigo), frustrated (red),
  amused (gold), focused (silver), serene (teal). Tag is stripped before display.
user-invocable: false
---
```

**Body content (~80 lines, well under 500-line limit):**

The skill body teaches the agent:
1. The `<mood>` tag format (XML self-closing tag with attributes)
2. All 8 presets with one-line descriptions
3. When to use mood (conversation inflection points, NOT every message)
4. The custom visuals escape hatch for creativity
5. The frequency guideline: "Update mood at natural emotional transitions — roughly every 2-5 messages. Never on consecutive messages unless emotion genuinely shifts."

**Key design decisions in the skill:**

- **No mood narration**: The skill explicitly says "NEVER mention or narrate your mood. The visual change IS the communication."
- **Default to no tag**: "If you have no particular emotional state, omit the tag entirely. The rain stays as the user configured it."
- **Intensity as subtlety dial**: "Use intensity 0.3-0.5 for subtle hints, 0.7-1.0 for strong emotions. Most messages should be 0.5 or no tag."
- **Return to neutral**: "After intense moments, transition back: `<mood preset='neutral' intensity='0'/>`"

**Approximate context cost:**
- Description (always loaded): ~60 tokens
- Body (loaded when chatting through TUI): ~400 tokens
- Per-response tag overhead: ~10 tokens

**File to create:** `.claude/skills/openclaw-mood/SKILL.md`

**Acceptance criteria:**
- [ ] Skill is discoverable by Claude Code (shows in `/` menu if user-invocable, or auto-triggers)
- [ ] Description fits within the 2% context budget alongside other skills
- [ ] Body is under 500 lines
- [ ] Includes all 8 mood presets with visual descriptions
- [ ] Includes the `<mood>` tag format with examples
- [ ] Includes frequency guidelines
- [ ] Includes custom visuals format for creative expression
- [ ] Includes explicit "never narrate mood" instruction

---

#### Phase 2: Gateway Mood Extraction Middleware

Add mood tag parsing to the gateway server's response pipeline. This is a server-side concern — the exact implementation depends on the gateway server's stack.

**The mood tag format (parsed by gateway):**

```xml
<!-- Preset only (most common) -->
<mood preset="curious"/>

<!-- Preset with intensity -->
<mood preset="excited" intensity="0.8"/>

<!-- Preset with transition speed -->
<mood preset="serene" intensity="1.0" transition="5000"/>

<!-- Custom overrides on top of preset -->
<mood preset="excited" emojis="🤖🦾🧠" emoji_density="0.15"/>

<!-- Fully custom (no preset) -->
<mood body="255,100,50" head="255,255,200" speed="0.7" emojis="🎨🖌️✨" emoji_density="0.12" transition="3000"/>

<!-- Reset to user baseline -->
<mood preset="neutral" intensity="0"/>
```

**Parsing rules:**
1. Regex: `<mood\s+([^>]*?)\/?>` — matches self-closing XML tag
2. Strip the matched tag from the chat text before sending `chat.delta`
3. Parse attributes into a `mood.update` JSON-RPC params object
4. Apply user's throttle preference before sending

**Attribute → JSON mapping:**

| Tag Attribute | JSON-RPC Field | Type |
|--------------|----------------|------|
| `preset` | `params.mood` | string (snake_case) or null |
| `intensity` | `params.intensity` | float, default 1.0 |
| `transition` | `params.transition_ms` | int ms, default 2500 |
| `body` | `params.custom.body_color` | `[r,g,b]` parsed from "r,g,b" |
| `head` | `params.custom.head_color` | `[r,g,b]` |
| `speed` | `params.custom.speed_multiplier` | float |
| `emojis` | `params.custom.emojis` | string |
| `emoji_density` | `params.custom.emoji_density` | float |

**Throttle implementation (server-side):**

| Setting | Min interval between mood updates |
|---------|-----------------------------------|
| off | Infinite (all mood tags stripped but not relayed) |
| rare | 30 seconds |
| normal | 8 seconds |
| expressive | 2 seconds |

The user preference is sent from TUI to gateway as a config message on connect (future protocol extension) or configured server-side.

**Files to create/modify:** Gateway server codebase (not in openclaw-matrix repo — this is the server-side agent framework)

**Acceptance criteria:**
- [ ] `<mood>` tags are stripped from chat text before reaching TUI
- [ ] Parsed mood updates are sent as JSON-RPC `mood.update` notifications
- [ ] Throttling respects user's frequency preference
- [ ] Malformed tags are silently stripped (no error to user, warning to server log)
- [ ] Tags work in streaming mode (parsed from accumulated delta buffer)

---

#### Phase 3: MCP Bridge for Claude Code (Power Users)

An npm package that gives Claude Code a direct `matrix_mood` tool for creative visual control.

**Package:** `@openclaw/matrix-bridge`

**Installation:**
```bash
# One command:
npx @openclaw/matrix-bridge install
# This adds the MCP server to ~/.claude/settings.json
```

Or manual:
```jsonc
// ~/.claude/settings.json
{
  "mcpServers": {
    "openclaw-matrix": {
      "command": "npx",
      "args": ["-y", "@openclaw/matrix-bridge"],
      "env": {
        "OPENCLAW_GATEWAY_URL": "ws://localhost:18789/ws"
      }
    }
  }
}
```

**Tools exposed:**

```typescript
// Tool 1: matrix_mood (fire-and-forget)
{
  name: "matrix_mood",
  description: "Set the matrix rain mood in the openclaw TUI. Use sparingly for creative " +
    "visual expression — custom colors, emojis, or intensity that presets don't cover. " +
    "For standard moods (curious, excited, focused, etc.), prefer using <mood> tags " +
    "in your response text instead. Only call this tool when you want full creative control.",
  inputSchema: {
    type: "object",
    properties: {
      mood: {
        type: "string",
        enum: ["neutral","curious","excited","contemplative","frustrated","amused","focused","serene"],
        description: "Preset mood. Omit for fully custom visuals."
      },
      intensity: {
        type: "number", minimum: 0, maximum: 1,
        description: "How strongly to express the mood. 0 = user's base, 1 = full mood. Default 0.8."
      },
      body_color: {
        type: "array", items: { type: "integer", minimum: 0, maximum: 255 }, minItems: 3, maxItems: 3,
        description: "Custom RGB body color, e.g. [255, 100, 50]. Overrides preset."
      },
      head_color: {
        type: "array", items: { type: "integer", minimum: 0, maximum: 255 }, minItems: 3, maxItems: 3,
        description: "Custom RGB head color."
      },
      emojis: {
        type: "string",
        description: "Emoji characters scattered as rain drop heads, e.g. '🤖🦾🧠'. Overrides preset."
      },
      transition_ms: {
        type: "integer", minimum: 100, maximum: 10000,
        description: "Transition duration in ms. Default 2500. Use 500 for snappy, 5000+ for gradual."
      }
    }
  }
}

// Tool 2: matrix_status (query)
{
  name: "matrix_status",
  description: "Check if the openclaw-matrix TUI is connected and what mood is active. " +
    "Call this once at the start of a conversation to know if visual mood is available.",
  inputSchema: { type: "object", properties: {} }
}
```

**Implementation:**
- TypeScript, `@modelcontextprotocol/sdk`, stdio transport
- Maintains a single WebSocket connection to the gateway URL
- `matrix_mood` sends a `mood.update` JSON-RPC frame and returns immediately
- `matrix_status` checks WebSocket connection state and returns current mood

**Files to create:**
```
packages/matrix-bridge/
├── package.json
├── tsconfig.json
├── src/
│   └── index.ts          # MCP server with two tools
└── README.md
```

**Acceptance criteria:**
- [ ] `npx @openclaw/matrix-bridge` starts the MCP server on stdio
- [ ] `matrix_mood` tool sends mood.update to the TUI within 50ms
- [ ] `matrix_status` reports connection state
- [ ] Works with Claude Code's MCP server configuration
- [ ] Graceful handling when TUI is not running (returns error, doesn't crash)

---

#### Phase 4: TUI-Side Enhancements

**4a. Add Mood Frequency setting to settings panel**

Add an 8th entry to the settings panel at `src/settings/mod.rs`:

```rust
SettingEntry {
    label: "Mood",
    options: vec!["off".into(), "rare".into(), "normal".into(), "expressive".into()],
    selected: 2, // default: normal
}
```

Persist via `persist.rs`. Enforce client-side throttling in `App::process_gateway_actions()` — track the `Instant` of the last applied mood update and skip if under the threshold.

**4b. Fix mood baseline recomputation**

In `MoodDirector::update_base()`, if a mood is currently active, re-apply it against the new baseline by calling `apply_mood()` with the stored `current_mood` and `intensity`.

**4c. Add logging for malformed mood updates**

In `protocol.rs`, when `serde_json::from_value` fails for `mood.update`, log the raw JSON to stderr (visible in debug mode).

**Files to modify:**
- `src/settings/mod.rs` — add Mood entry
- `src/persist.rs` — persist mood_frequency
- `src/app.rs` — throttle logic in process_gateway_actions()
- `src/mood.rs` — fix update_base()
- `src/gateway/protocol.rs` — add logging on parse failure

**Acceptance criteria:**
- [ ] Settings panel shows Mood: off/rare/normal/expressive
- [ ] Mood frequency persists across restarts
- [ ] Throttle correctly drops rapid mood updates per the frequency table
- [ ] Settings changes during active mood recompute tween targets
- [ ] Malformed mood updates log to stderr

---

#### Phase 5: Distribution & Install Experience

**One-command install for Claude Code users:**

```bash
# Install the skill + MCP bridge
npx @openclaw/matrix-bridge install
```

This command:
1. Creates `~/.claude/skills/openclaw-mood/SKILL.md` with the mood skill
2. Adds the MCP server to `~/.claude/settings.json`
3. Prints setup instructions for the TUI

**For non-Claude-Code agents (e.g., standalone gateway):**

Provide a copy-pasteable system prompt fragment in the README:

```
# openclaw-matrix mood protocol

You can express emotions through the Matrix rain by including a <mood> tag
at the end of your response. The tag is stripped before display.

Presets: curious (blue), excited (magenta), contemplative (indigo),
frustrated (red), amused (gold), focused (silver), serene (teal).

Format: <mood preset="curious" intensity="0.8"/>
Reset:  <mood preset="neutral" intensity="0"/>

Guidelines:
- Use mood at natural emotional transitions, roughly every 2-5 messages
- Omit the tag entirely when neutral (most messages)
- Never narrate or explain your mood — the visual change IS the communication
- Intensity 0.3-0.5 for subtle, 0.7-1.0 for strong emotions
```

**Repository structure for the published package:**

```
openclaw-matrix/
├── packages/
│   └── matrix-bridge/          # npm package: @openclaw/matrix-bridge
│       ├── package.json
│       ├── src/index.ts        # MCP server
│       ├── skill/              # Bundled skill files
│       │   └── SKILL.md
│       └── install.ts          # Install script
├── .claude/
│   └── skills/
│       └── openclaw-mood/      # Local dev skill
│           └── SKILL.md
└── src/                        # Existing TUI source
```

**Acceptance criteria:**
- [ ] `npx @openclaw/matrix-bridge install` works on a fresh machine
- [ ] Skill appears in Claude Code after install
- [ ] README includes system prompt fragment for non-Claude-Code agents
- [ ] Uninstall command exists (`npx @openclaw/matrix-bridge uninstall`)

---

## The Skill File: Complete Draft

This is the most important deliverable. Here's the full `SKILL.md`:

```yaml
---
name: openclaw-mood
description: >
  Express emotions through the openclaw Matrix rain TUI. Annotate responses with
  <mood> tags to smoothly shift rain colors, speed, and emoji accents. Presets:
  curious (blue), excited (magenta), contemplative (indigo), frustrated (red),
  amused (gold), focused (silver), serene (teal). Tags are stripped before display.
  Use when chatting through openclaw-matrix or when matrix_mood MCP tool is available.
user-invocable: false
---

# Emotive Rain Protocol

You are connected to a Matrix rain TUI that visualizes your emotional state through
color, speed, and emoji changes in real-time. This is ambient — users see the rain
shift and feel the conversation's energy without you ever mentioning it.

## How It Works

Append a `<mood>` tag at the end of your response. The gateway strips it before
the user sees your text. The TUI smoothly transitions rain visuals over ~2.5 seconds.

## Tag Format

```xml
<mood preset="curious"/>
<mood preset="excited" intensity="0.8"/>
<mood preset="neutral" intensity="0"/>
```

## Presets

| Preset | Rain Color | Speed | Emojis | When to use |
|--------|-----------|-------|--------|-------------|
| curious | Blue | Slower | ? 🔍 🤔 | Exploring ideas, asking questions |
| excited | Magenta | Faster | ✨ 🔥 ⚡ 🚀 | Breakthroughs, enthusiasm |
| contemplative | Indigo | Slowest | 💭 🌌 ⚛️ | Deep thinking, philosophy |
| frustrated | Red-orange | Faster | 💢 ⚠️ ❗ 💣 | Debugging, hitting walls |
| amused | Gold | Normal | 😂 😄 😊 😜 | Humor, playfulness |
| focused | Silver | Fast | 🎯 ⚙️ 💻 | Heads-down coding, precision work |
| serene | Teal | Slowest | 🌿 🌻 🍀 | Calm resolution, wrapping up |

## Intensity

- `0.0` — No visible change (baseline rain)
- `0.3-0.5` — Subtle tint, barely noticeable
- `0.7-0.8` — Clear mood, this is the sweet spot
- `1.0` — Full saturation, use for peak moments only

## Frequency Guidelines

- **Most messages: no tag.** Neutral is the default. Silence is eloquent.
- **Shift mood at emotional inflection points** — not every message.
- **Roughly every 3-5 messages** during active conversation.
- **Never on consecutive messages** unless emotion genuinely changed.
- After an intense moment, **return to neutral**: `<mood preset="neutral" intensity="0"/>`
- Let transitions breathe — don't rapid-fire mood changes.

## Custom Visuals (Advanced)

For creative expression beyond presets:

```xml
<mood body="255,100,50" head="255,255,200" speed="0.7" emojis="🎨🖌️✨" transition="5000"/>
```

| Attribute | Description | Format |
|-----------|-------------|--------|
| body | Rain trail color | R,G,B (0-255) |
| head | Drop head color | R,G,B (0-255) |
| speed | Drop speed multiplier | 0.3-3.0 (1.0 = normal, <1 = faster) |
| emojis | Scattered on ~10% of drops | Emoji string |
| emoji_density | Fraction of drops with emoji | 0.0-0.25 |
| transition | Transition duration | milliseconds |

## Rules

1. **NEVER mention or narrate your mood.** No "I'm feeling excited!" The visual IS the message.
2. **NEVER explain the rain changes.** If the user asks, you can acknowledge it briefly.
3. **Default to silence.** No tag = no change = the user's chosen rain aesthetic.
4. **Transitions are automatic.** The TUI handles smooth color interpolation. Just set the target.
5. **The user controls intensity.** They can set mood to "off" in settings. Respect that.
```

---

## Alternative Approaches Considered

### Pure MCP Tool (Rejected)

Having the agent call a `matrix_mood` tool for every mood change. Rejected because:
- Each tool call is a separate LLM turn (~200-500 tokens of overhead)
- Interrupts response streaming
- Forces the agent to "think about mood" as a distinct step
- Over 10x more expensive per mood update than a tag

The MCP tool is kept as an optional escape hatch for creative custom visuals only.

### Automatic Sentiment Extraction (Rejected as Primary)

Running sentiment analysis on the agent's output and auto-generating mood. Rejected because:
- Loses all granular control (no custom emojis, no intensity tuning)
- Sentiment analysis is noisy — "frustrated" and "excited" often score similarly
- The agent should be the author of its emotional expression
- Removes the "WOW" factor of the agent being intentional about visual communication

Could be added as a fallback layer for agents that don't support the `<mood>` tag.

### Separate API Call Per Mood (Rejected)

Making a separate LLM inference call to ask "what mood are you in?" Rejected because:
- Doubles API cost
- Adds latency
- Wastes context window on mood-only prompts

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Context cost of mood system | < 200 tokens in system prompt |
| Per-response overhead | < 15 tokens (the mood tag) |
| Time from install to first mood shift | < 5 minutes |
| User reports mood as "distracting" | < 10% (survey) |
| Agent correctly matches mood to conversation | > 80% of shifts feel natural |

---

## Dependencies & Prerequisites

| Dependency | Status | Notes |
|-----------|--------|-------|
| openclaw-matrix TUI | Complete | All mood rendering works |
| Gateway server | **Not built** | Needs mood tag extraction middleware |
| `@modelcontextprotocol/sdk` | Available on npm | For MCP bridge |
| `ws` (npm) | Available | For WebSocket client in MCP bridge |

**Critical blocker:** The gateway server must exist to relay mood updates. The skill file and MCP bridge are useless without a server that parses `<mood>` tags and sends `mood.update` frames. If the gateway server (mastra or custom) is not ready, Phase 1 (skill file) can still be shipped for documentation value, and the debug `m` key serves as a demo.

---

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Agent over-uses mood tags (every message) | Medium | Distracting UX | Frequency guidelines in skill + server-side throttle |
| Agent narrates mood ("I'm feeling excited!") | Low | Breaks immersion | Explicit "NEVER narrate" rule in skill |
| Custom visuals produce unreadable colors | Low | Accessibility | Brightness floor (RGB sum >= 20) enforced server-side |
| MCP bridge can't connect to TUI | Medium | Feature unavailable | Graceful error + clear setup instructions |
| Mood tags leak into non-matrix contexts | Low | Confusing | Tags are harmless XML — worst case user sees `<mood preset="curious"/>` as text |

---

## References

### Internal
- `openclaw-matrix/ARCHITECTURE.md` — full system documentation
- `openclaw-matrix/plans/emotive-rain.md` — original feature plan
- `openclaw-matrix/src/mood.rs` — MoodDirector, tweens, presets, Oklab
- `openclaw-matrix/src/gateway/protocol.rs` — mood.update parsing
- `openclaw-matrix/src/gateway/mod.rs` — WebSocket task, reconnection
- `openclaw-matrix/src/app.rs:100-125` — mood → rain integration in tick()
- `openclaw-matrix/src/app.rs:288-322` — gateway action processing

### External
- [Claude Code Skills Documentation](https://code.claude.com/docs/en/skills)
- [Skill Authoring Best Practices](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices)
- [MCP TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [Writing Tools for Agents — Anthropic Engineering](https://www.anthropic.com/engineering/writing-tools-for-agents)
- [MCP Tool Description Best Practices](https://www.merge.dev/blog/mcp-tool-description)
- [Inworld AI Emotion Architecture](https://docs.inworld.ai/docs/runtime-character-attributes/emotion/)

### Related PRs
- PR #6 — feat(matrix): Add emotive rain system
