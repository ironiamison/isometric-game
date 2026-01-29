# NPC Speech Bubbles Design

## Goal

Make the world feel more lively by having NPCs randomly say messages when players are nearby. Mix of ambient flavor text and role-based hints to help players discover interactable NPCs.

## Data Definition

Add a `[speech]` block to NPC TOML prototypes:

```toml
[elder_villager.speech]
radius = 5
interval_min_ms = 15000
interval_max_ms = 45000
messages = [
    "The old forest isn't what it used to be...",
    "Adventurer, I could use your help.",
    "I remember when this village was just a few huts.",
]
```

- `radius` — tile distance for both proximity trigger and broadcast range
- `interval_min_ms` / `interval_max_ms` — random wait range between lines
- `messages` — list of strings the NPC randomly picks from
- NPCs with no `[speech]` block never speak

## Server Logic

Each NPC with a `speech` config tracks a `next_speech_at` timestamp. On each game tick:

1. **Proximity check** — Are any players within `radius` tiles? If not, reset timer and skip.
2. **Timer check** — Has `next_speech_at` been reached? If not, skip.
3. **Pick message** — Select a random message from the list.
4. **Broadcast** — Send `NpcSpeech { npc_id, message }` to all players within `radius`.
5. **Reset timer** — Set `next_speech_at` to now + random(interval_min_ms, interval_max_ms).

The timer only counts down while players are nearby. When all players leave, the timer resets to avoid speech bursts on re-approach.

## Client Rendering

Reuse the existing player chat bubble system:

1. Add `speech_bubble: Option<ChatBubble>` to the client-side NPC struct.
2. On receiving `NpcSpeech { npc_id, message }`, store it on the NPC with a timestamp.
3. Extend `render_chat_bubbles()` to also draw bubbles above NPCs.
4. Same 5-second duration with 1-second fade-out, matching player chat bubbles.

## Protocol

New server message:

```
ServerMessage::NpcSpeech { npc_id: String, message: String }
```

No new client messages needed — this is entirely server-driven.
