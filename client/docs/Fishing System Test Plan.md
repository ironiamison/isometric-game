Fishing System Test Plan

1. Skill System Basics

- Load an existing character → Fishing skill should appear at level 1 with 0 XP
- Create a new character → Fishing skill starts at level 1
- Skills panel shows Hitpoints, Combat, and Fishing (3 active, 5 locked)
- Total level includes Fishing (e.g. HP 10 + Combat 3 + Fishing 1 = 14)

2. Starting Gathering

- Walk to a fishing marker tile (around 10-12, 25-26) and send startGathering → receive gatheringStarted with zone_id "pond"
- Try starting on a non-marker tile → receive error
- Try starting on an occupied marker (another player there) → receive "already occupied" error
- Try starting at a zone requiring higher level (river=15, ocean=40) at level 1 → receive level requirement error

3. Gathering Loop

- Stand on a pond marker and start gathering → after ~5 seconds, receive gatheringResult with a fish item and XP
- Confirm inventory receives the fish item (raw_shrimp or raw_sardine at level 1)
- Confirm skillXp message arrives for fishing skill
- Gathering continues automatically — second fish should arrive ~5s after the first
- At level 5+, raw_sardine should start appearing in catches
- At level 10+, raw_herring can drop (uncommon tier)

4. Stopping Gathering

- Move while gathering → receive gatheringStopped with reason "moved", marker is freed
- Send stopGathering while gathering → receive gatheringStopped with reason "cancelled"
- Fill inventory to 20/20, then gather → receive gatheringStopped with reason "inventory_full"
- After stopping, another player can claim the same marker

5. XP and Leveling

- Gather fish and confirm XP accumulates (pond base_xp=10 + item xp_bonus)
- Accumulate 83+ XP → Fishing levels up to 2, skillLevelUp message broadcasts
- After leveling, higher-tier fish become eligible in loot rolls
- Save and reload character → Fishing XP and level persist correctly

6. Loot Table Tiers

- At level 1 at pond: only raw_shrimp drops (common tier, only item with level≤1)
- At level 5: raw_sardine also eligible (common tier)
- At level 10: raw_herring eligible (uncommon tier)
- At level 20: raw_trout eligible (rare tier)
- Higher player level shifts tier weights: common decreases (-0.5/level), uncommon/rare increase

7. Bonus Tiles

- After ~60 seconds at a pond zone, bonusTileSpawned broadcasts with 5000ms telegraph
- If unclaimed after 5s, bonusTileExpired broadcasts
- Walk to bonus tile position and it should be claimable → bonusTileClaimed + buffApplied with 30s duration
- While buffed, gather speed doubles (2.5s ticks instead of 5s)
- After 30s, buff expires and speed returns to normal

8. Multiplayer

- Two players at different markers in the same zone → both gather independently
- One player per marker enforced — second player gets "already occupied"
- Player disconnects or moves → marker frees up immediately
- Bonus tile: first player to reach it claims the buff, others cannot

9. Edge Cases

- Start gathering, then attack/get attacked → gathering should continue (combat doesn't auto-cancel)
- Die while gathering → need to verify behavior (player respawns, marker should free)
- Enter a portal while gathering → should stop gathering
- /give command to test higher-level fish items exist in registry
- Server restart → all gathering state resets (markers freed, no active gatherers)
