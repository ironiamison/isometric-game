# Phase 2 Local Test Plan

This plan covers the shared resource contract system for farming, mining, woodcutting, fishing, and smithing, plus the main regressions around NPC interaction flow and the custom Adventure Board UI.

## Local Setup

Start the server:

```bash
cd rust-server
cargo run --release
```

Start the client:

```bash
cd client
cargo run --release
```

Use two character states if possible:

- `Fresh character`
  - For tutorial-gating checks on Miner Mike and Lumberjack Pete.
- `Established character`
  - For fast farming/mining/woodcutting/fishing checks, especially medium or hard contracts if you already have level 15+ or 30+.

If the board spawn is missing locally for any reason, you can create one with the admin command:

```text
/spawn adventure_board
```

## Pass Criteria

- Only one resource contract can be active at a time across farming, mining, woodcutting, fishing, and smithing.
- Farming, mining, woodcutting, fishing, and smithing contracts all use the same tracker UI.
- Miner Mike and Lumberjack Pete still behave like tutorial NPCs before their first quest is completed.
- Miner Mike and Lumberjack Pete still open their shops after the tutorial quest is completed.
- The Adventure Board can issue shared contracts without visiting skill NPCs.
- The Adventure Board opens a custom panel instead of a generic dialogue box.
- The board panel shows lane selection, per-difficulty rewards, the current active contract, and cumulative board totals.
- Contract progress persists across reconnects.
- Smithing contracts progress when crafted outputs are produced, for both instant and timed crafts.

## Test Cases

### 1. Tutorial gating for Miner Mike

1. Log in on a fresh character that has not completed `rock_bottom`.
2. Talk to Miner Mike.

Expected:

- You get the existing mining tutorial flow.
- You do not get the new contract/shop split dialogue yet.

### 2. Tutorial gating for Lumberjack Pete

1. Log in on a fresh character that has not completed `axe_to_grind`.
2. Talk to Lumberjack Pete.

Expected:

- You get the existing woodcutting tutorial flow.
- You do not get the new contract/shop split dialogue yet.

### 3. Post-tutorial Miner Mike interaction

1. Complete `rock_bottom`.
2. Talk to Miner Mike again.
3. Choose `Open shop`.
4. Talk to Miner Mike again and choose `Show contracts`.

Expected:

- After quest completion, Miner Mike shows a dialogue with `Show contracts`, `Open shop`, and `Nevermind`.
- `Open shop` still opens the mining merchant normally.
- `Show contracts` opens the mining contract screen.

### 4. Post-tutorial Lumberjack Pete interaction

1. Complete `axe_to_grind`.
2. Talk to Lumberjack Pete again.
3. Choose `Open shop`.
4. Talk to Lumberjack Pete again and choose `Show contracts`.

Expected:

- After quest completion, Lumberjack Pete shows a dialogue with `Show contracts`, `Open shop`, and `Nevermind`.
- `Open shop` still opens the woodcutting merchant normally.
- `Show contracts` opens the woodcutting contract screen.

### 5. Master Farmer regression

1. Talk to Master Farmer.
2. Choose `Buy allotment plot`.
3. Reopen dialogue and choose `Farming contracts`.

Expected:

- Plot purchase dialogue still opens normally.
- Farming contract dialogue still opens normally.
- Master Farmer still behaves as the farming hub NPC.

### 6. Adventure Board spawn and interaction

1. Log in near the starting village.
2. Find the `Adventure Board` near the spawn/tutorial area.
3. Interact with it.

Expected:

- The board opens a custom contract panel.
- The panel shows:
  - lane cards for each contract skill
  - reward rows for easy, medium, and hard jobs
  - a current active contract panel
  - cumulative totals such as completed jobs, total XP, and total gold earned
- It offers at least:
  - farming
  - mining
  - woodcutting
  - fishing
  - smithing

### 7. Adventure Board lane selection and rewards

1. Open the `Adventure Board`.
2. Click through each lane card.
3. Inspect the listed easy, medium, and hard options.

Expected:

- The selected lane changes immediately.
- Each lane shows its own skill level and description.
- Each difficulty row shows reward XP and gold.
- Locked rows clearly show their required level.
- If you already have an active contract, take-contract actions are visually disabled.

### 8. Accept an easy farming contract

1. On any character with Farming 1+, talk to Master Farmer.
2. Choose `Farming contracts`.
3. Accept an easy contract.

Expected:

- You receive a system message confirming the contract.
- The tracker appears on the left side of the HUD.
- The tracker shows:
  - difficulty
  - contract skill
  - task text
  - progress count
  - return target NPC when complete

### 9. Accept an easy mining contract

1. On a character with `rock_bottom` completed and Mining 1+, talk to Miner Mike.
2. Choose `Show contracts`.
3. Accept an easy contract.

Expected:

- Contract accepts successfully.
- Tracker appears even outside the farming area.
- Tracker text uses mining-specific wording such as `Mine ...` and `... mined`.

### 10. Accept an easy woodcutting contract

1. On a character with `axe_to_grind` completed and Woodcutting 1+, talk to Lumberjack Pete.
2. Choose `Show contracts`.
3. Accept an easy contract.

Expected:

- Contract accepts successfully.
- Tracker text uses woodcutting-specific wording such as `Chop ...` and `... chopped`.

### 11. Accept an easy fishing contract from the board

1. Talk to the `Adventure Board`.
2. Choose the fishing job branch.
3. Accept an easy fishing contract.
4. Catch the requested fish.

Expected:

- Contract accepts successfully.
- Tracker text uses fishing-specific wording such as `Catch ...` and `... caught`.
- Progress updates only when the matching fish is received.
- Fishing XP and normal item gain still work.

### 12. One-active-contract rule

1. Accept any one resource contract.
2. Without finishing or abandoning it, talk to one of the other contract NPCs.

Expected:

- The second NPC does not offer a second active contract.
- The dialogue explains that you already have an active resource contract.
- You can abandon the current one from the dialogue if needed.

### 13. Accept an easy smithing contract from the board

1. Talk to the `Adventure Board`.
2. Choose the smithing job branch.
3. Accept an easy smithing contract.
4. Craft the requested item at the required station.

Expected:

- Contract accepts successfully.
- Tracker text uses smithing-specific wording such as `Smith ...` and `... crafted`.
- Progress updates only when the crafted output matches the contract target.
- Smithing XP and normal crafted item gain still work.

### 14. Farming progress updates

1. Accept a farming contract.
2. Harvest the requested crop.

Expected:

- Progress increases only when the harvested produce matches the contract target.
- System messages update progress.
- Tracker progress updates immediately.
- When the required amount is reached, you get a completion message telling you to return to the giver.

### 15. Mining progress updates

1. Accept a mining contract.
2. Mine the requested ore until at least one matching item is received.

Expected:

- Progress increases only when the mined ore matches the contract target.
- Normal ore rewards and mining XP still work.
- Contract tracker updates immediately after successful ore collection.

### 16. Woodcutting progress updates

1. Accept a woodcutting contract.
2. Chop the requested logs until at least one matching item is received.

Expected:

- Progress increases only when the chopped log matches the contract target.
- Normal log rewards and woodcutting XP still work.
- Contract tracker updates immediately after successful log collection.

### 17. Smithing progress updates for timed crafts

1. Accept a smithing contract for an item that uses a timed smithing recipe, if available on your character.
2. Start the craft.
3. Let the timer complete.

Expected:

- Progress updates when the crafted item is awarded at completion.
- The contract does not update early when the craft merely starts.
- Normal smithing XP, inventory updates, and crafting-complete messages still work.

### 18. Completion and reward claim

Run this once for each contract type you want to verify.

1. Finish the contract objective.
2. Return to the giver.
3. Claim the reward.

Expected:

- The contract can be claimed from the dialogue.
- The contract can also be claimed from the Adventure Board if it was taken there.
- The tracker clears after claim.
- Gold reward is granted.
- Skill XP is granted to the correct skill:
  - Farming contracts grant Farming XP
  - Mining contracts grant Mining XP
  - Woodcutting contracts grant Woodcutting XP
  - Fishing contracts grant Fishing XP
  - Smithing contracts grant Smithing XP
- Farming contracts also grant bonus seeds.

### 19. Board claim and abandon flow

1. Take a contract from the `Adventure Board`.
2. Make partial progress.
3. Reopen the board and inspect the same contract branch.
4. Abandon it.

Expected:

- The board can display your current active contract even if it was started earlier.
- The board progress panel updates after accept, abandon, and claim.
- Abandon removes it immediately.
- You can take a different contract from the board right after.

### 20. Abandon flow

1. Accept a contract.
2. Partially progress it.
3. Reopen the contract dialogue and abandon it.

Expected:

- Contract is removed immediately.
- Tracker disappears.
- You can accept a different contract afterward.

### 21. Reconnect persistence

1. Accept a contract.
2. Make partial progress.
3. Close the client and reconnect.

Expected:

- The active contract is restored on login.
- The tracker still shows the same task and progress.

Repeat once with a fully completed but unclaimed contract.

Expected:

- The contract still shows as complete after reconnect.
- You can still claim it normally.

### 22. Existing-character migration check

Optional, only if you already had a farming contract on a saved character before this patch.

1. Log in on that character after updating the server.
2. Check the tracker and contract progress.
3. Restart the server and reconnect again.

Expected:

- The old farming contract still exists as a resource contract.
- It does not duplicate on restart.
- Progress, abandon, and claim still work.

### 23. Phase 1 regression spot check

1. Talk to Wise Man, Camp Cook, and Elder Mara on a character that meets their quest prerequisites.
2. Confirm the repeatable quest entries still appear in NPC order.

Expected:

- The repeatable quest flow from Phase 1 is unchanged.
- Resource contracts did not break quest selection order or repeatable quest access.

## Notes During Testing

- Easy contracts are the minimum required validation.
- Medium contracts should unlock at skill level 15.
- Hard contracts should unlock at skill level 30.
- If you only have low-level characters locally, complete the easy-path tests first and treat medium/hard as follow-up coverage.
