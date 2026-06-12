# Content Roadmap

This roadmap focuses on retention content that fits the current Aeven codebase instead of introducing large new systems too early.

## Goals

- Give players a reason to log in even during short sessions.
- Reuse existing quest, slayer, farming, KOTH, and leaderboard systems.
- Favor content that works with 1 player online as well as with a crowd.
- Avoid building loops that depend on synchronous PvP participation.

## Current System Fit

- `rust-server/src/quest/*` already supports repeatable quests, mixed objective types, Lua dialogue, and sequential turn-ins.
- `rust-server/src/farming/contracts.rs` already provides a strong short-session contract pattern.
- `rust-server/src/koth.rs` already provides scalable wave-event logic with reward checkpoints.
- `rust-server/data/slayer/*` already provides combat task assignment, unlocks, and reward sinks.
- The current arena depends on queue concurrency and is better positioned as event content than as the primary retention loop.

## Phase 1: Repeatable Jobs

Status: in progress in this patch.

Objective:
Create a reliable low-friction loop that players can pick up from existing quest NPCs.

Implementation:

- Add one repeatable quest to non-merchant NPCs so we do not block shop access.
- Keep requirements low enough for early and mid-game characters.
- Use inventory-backed objectives where possible so rewards feel earned and the loop interacts with gathering, cooking, and combat.
- Keep turn-ins short and readable with existing dialogue UI.

Initial pack:

- `wise_man`: mixed village supply run using woodcutting, mining, and fishing/cooking outputs.
- `camp_cook`: repeatable cooking loop after the survivalist tutorial chain.
- `elder_villager`: repeatable cursed-lands cleanup loop after the first area-introduction quest.
- `farmer_grace`: existing repeatable remains part of the same content lane.

Technical notes:

- Quest ordering for NPC interaction should be deterministic and follow NPC `available_quests` order when present.
- Repeatables that should only appear after a tutorial chain should use `quest.chain.previous`.
- Merchant NPCs should not receive repeatables until merchant/quest interaction rules are redesigned.

Success criteria:

- A new player can finish tutorials and immediately access at least one repeatable loop.
- A returning player can grab a job, play for 5 to 15 minutes, and log out with visible progress.
- No new client protocol or bespoke UI is required.

## Phase 2: Contract Expansion

Objective:
Turn the farming contract pattern into a broader profession-job system.

Implementation:

- Generalize contract generation for mining, woodcutting, fishing, smithing, and alchemy.
- Reuse contract tracker UI patterns from farming where practical.
- Add tiered rewards by skill level and location.
- Add light persistence and one active contract per skill family or one total contract, depending on friction observed.

Recommended file targets:

- `rust-server/src/game/farming/contracts.rs` as the reference flow.
- New server modules under `rust-server/src/game/` for each contract family.
- Client tracker work near `client/src/render/renderer.rs`.

Success criteria:

- Each major non-combat skill has a short repeatable job loop.
- Players can choose a focused skilling session instead of only freeform grinding.

## Phase 3: Public PvE Events

Objective:
Create shared activity that scales down to solo participation and up to group participation.

Implementation:

- Fork KOTH-style wave logic into overworld or instanced event variants.
- Add event templates such as caravan defense, corruption breach, obelisk defense, or village siege.
- Broadcast event state through existing messaging patterns before building bespoke UI.
- Reward materials, cosmetics, and progress tokens instead of only raw gold.

Recommended file targets:

- `rust-server/src/koth.rs`
- `rust-server/src/game/koth_tick.rs`
- new event modules under `rust-server/src/game/`

Success criteria:

- Events are understandable without a manual.
- Solo players can contribute.
- Group participation improves efficiency without becoming mandatory.

## Phase 4: Slayer as the Main Combat Treadmill

Objective:
Make slayer the long-term combat retention loop.

Implementation:

- Add more zone-specific tasks and unlocks.
- Add streak bonuses and better milestone rewards.
- Add task-only drops and slayer-exclusive cosmetic prestige items.
- Add more reward sinks to spend slayer points on consistently.

Recommended file targets:

- `rust-server/data/slayer/masters.toml`
- `rust-server/data/slayer/rewards.toml`
- `rust-server/src/slayer/*`

Success criteria:

- Combat progression feels directed instead of random.
- Midgame players have a long-tail grind with meaningful unlocks.

## Phase 5: Collection and Prestige

Objective:
Preserve the value of old content through account-wide long-term goals.

Implementation:

- Add collection log tracking for monster drops, fish, ores, bars, cooked foods, quest clears, and boss rewards.
- Add cosmetic titles, capes, or housing trophies for completion thresholds.
- Surface progress in web stats or a future in-game journal.

Recommended file targets:

- persistence in `rust-server/src/db.rs`
- player profile surfaces in `site/src/routes/world/player/[name]/+page.svelte`

Success criteria:

- Old content remains worth doing after its direct rewards flatten out.
- Completionists get durable goals without destabilizing combat balance.

## Arena Positioning

Do not treat the arena as the main retention loop until concurrency is consistently high enough.

Better near-term uses:

- weekend event format
- seasonal ladder
- wager exhibition matches
- opt-in tournament bracket nights

The arena can become strong social content later, but it should not be the first answer to low-pop retention.
