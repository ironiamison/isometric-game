# Trade & Stall System — Test Plan

## Setup

1. Swap `WS_URL` to `ws://localhost:3000` in both `client/src/main.rs` (line 32) and `client/src/app.rs` (line 22)
2. Start the server: `cd rust-server && cargo run`
3. Open **two** client instances: `cd client && cargo run` (in separate terminals)
4. Log in as two different characters (**Player A** and **Player B**)
5. Walk both characters within 3 tiles of each other

---

## 1. Direct Trade — Happy Path


| #   | Step                                             | Expected                                                                                                     |
| --- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| 1.1 | **A** right-clicks **B** → selects "Trade"       | B sees a popup: "[A's name] wants to trade" with Accept / Decline buttons                                    |
| 1.2 | **B** clicks **Accept**                          | Both clients open the trade window showing partner name, two offer columns, gold area, Accept/Cancel buttons |
| 1.3 | **A** clicks an inventory item                   | Item appears in A's "Your Offer" column, B sees it in "Their Offer"                                          |
| 1.4 | **A** clicks gold area → enters amount (e.g. 50) | A's gold offer shows 50g, B sees 50g in partner's offer                                                      |
| 1.5 | **B** clicks an inventory item                   | Item appears in B's offer, A sees it in partner's column                                                     |
| 1.6 | **A** clicks Accept                              | A's acceptance indicator shows ✓, B sees "Partner: ✓"                                                        |
| 1.7 | **B** clicks Accept                              | Trade completes — both windows close, items/gold are swapped. Chat message or notification confirms          |
| 1.8 | Verify inventories                               | A has B's item + lost 50g. B has A's item + gained 50g                                                       |


## 2. Trade — Decline & Cancel


| #   | Step                                             | Expected                                        |
| --- | ------------------------------------------------ | ----------------------------------------------- |
| 2.1 | **A** sends trade request to **B**               | B sees popup                                    |
| 2.2 | **B** clicks **Decline**                         | Popup closes. A gets "Trade declined" message   |
| 2.3 | Start a new trade, both accept, **A** adds items | Trade window open with offers                   |
| 2.4 | **A** clicks **Cancel**                          | Both trade windows close with "Trade cancelled" |


## 3. Trade — Accept Reset (Anti-Scam)


| #   | Step                                           | Expected                                          |
| --- | ---------------------------------------------- | ------------------------------------------------- |
| 3.1 | Open trade, A adds item, **both** click Accept | Both show ✓                                       |
| 3.2 | **A** adds another item (or removes one)       | **Both** accept flags reset to ✗ — must re-accept |
| 3.3 | **A** changes gold offer                       | Both accept flags reset again                     |
| 3.4 | Both re-accept                                 | Trade completes with the updated offers           |


## 4. Trade — Remove Items


| #   | Step                                             | Expected                                              |
| --- | ------------------------------------------------ | ----------------------------------------------------- |
| 4.1 | Open trade, A offers 2 different items           | Both items show in A's offer                          |
| 4.2 | A clicks on an offered item                      | Item removed from offer, returns to inventory display |
| 4.3 | Accept flags reset if either player had accepted | ✗ shown for both                                      |


## 5. Trade — Distance Cancel


| #   | Step                                        | Expected                                                             |
| --- | ------------------------------------------- | -------------------------------------------------------------------- |
| 5.1 | Open trade between A and B (within 3 tiles) | Trade window open                                                    |
| 5.2 | **A** walks more than 3 tiles away from B   | Trade auto-cancels. Both get "Too far apart." message, windows close |


## 6. Trade — Item Locking


| #   | Step                                                  | Expected                                                     |
| --- | ----------------------------------------------------- | ------------------------------------------------------------ |
| 6.1 | Open trade, A offers an inventory item                | Item is in the trade offer                                   |
| 6.2 | While trade is open, A tries to **drop** that item    | Server rejects — item stays in inventory. Error/chat message |
| 6.3 | A tries to **equip** that item                        | Server rejects                                               |
| 6.4 | A tries to **use** that item (eat food, etc.)         | Server rejects                                               |
| 6.5 | Items NOT in the trade offer can still be freely used | Normal behavior                                              |


## 7. Trade — Edge Cases


| #   | Step                                                         | Expected                                                        |
| --- | ------------------------------------------------------------ | --------------------------------------------------------------- |
| 7.1 | **B** disconnects during an open trade                       | A's trade window closes with "Trade cancelled"                  |
| 7.2 | A sends trade request, **B** doesn't respond for 20+ seconds | Request expires silently (popup disappears on B if still shown) |
| 7.3 | A tries to trade B while B **has a stall open**              | Rejected: "Player is running a shop" (or similar)               |
| 7.4 | A tries to trade B while A is **already in a trade**         | Rejected                                                        |
| 7.5 | A tries to send trade request to self                        | Nothing should happen / rejected                                |
| 7.6 | Trade with partner who is **dead**                           | Should cancel or be rejected                                    |
| 7.7 | Accept trade with **insufficient inventory space**           | Server should reject and not complete the trade                 |


---

## 8. Player Stall — Setup & Open


| #   | Step                                         | Expected                                                                                       |
| --- | -------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 8.1 | **A** presses **Y**                          | Stall setup panel opens                                                                        |
| 8.2 | A clicks inventory items while panel is open | Items are listed in stall slots (default price 1g)                                             |
| 8.3 | A clicks **"Open Shop"**                     | Server confirms stall is open. A becomes **immobile**. Overhead green name tag appears above A |
| 8.4 | A tries to walk while stall is active        | Movement is blocked — character doesn't move                                                   |
| 8.5 | Verify overhead indicator                    | B can see a green shop name label above A's character                                          |


## 9. Player Stall — Browsing & Buying


| #   | Step                                          | Expected                                                                                             |
| --- | --------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 9.1 | **B** right-clicks **A** (who has stall open) | Context menu shows "Browse Shop" option                                                              |
| 9.2 | B clicks "Browse Shop"                        | Stall browse panel opens showing A's listed items with prices                                        |
| 9.3 | B clicks an item in the browse panel          | Item is selected, quantity controls appear                                                           |
| 9.4 | B adjusts quantity with +/- buttons           | Total price updates (price × quantity)                                                               |
| 9.5 | B clicks **"Buy"** with enough gold           | Purchase succeeds. B receives item, B's gold decreases. A gets sale notification, A's gold increases |
| 9.6 | B tries to buy with **not enough gold**       | Purchase fails with error message                                                                    |
| 9.7 | B tries to buy more than available quantity   | Purchase fails or quantity clamped                                                                   |
| 9.8 | B tries to buy with **full inventory**        | Purchase fails: "Inventory full"                                                                     |
| 9.9 | Verify stall updates after purchase           | Item quantity in stall decreases. If sold out, slot becomes empty                                    |


## 10. Player Stall — Remove & Close


| #    | Step                                  | Expected                                                                                               |
| ---- | ------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| 10.1 | A clicks **"Remove"** on a stall slot | Item returns to A's inventory. Stall slot becomes empty                                                |
| 10.2 | A clicks **"Close Shop"**             | Stall closes. All remaining items return to inventory. A can move again. Overhead indicator disappears |
| 10.3 | Press **Y** to close the panel        | Setup panel closes                                                                                     |


## 11. Player Stall — Edge Cases


| #    | Step                                           | Expected                                                                                             |
| ---- | ---------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 11.1 | A opens stall, then **disconnects**            | Stall force-closes. Items return to A's inventory (overflow to bank). On reconnect, A has items back |
| 11.2 | A opens stall, then gets **killed** by a mob   | Stall auto-closes (tick loop check). Items return to inventory                                       |
| 11.3 | A tries to open stall while **in a trade**     | Rejected                                                                                             |
| 11.4 | A tries to open stall while **in an instance** | Rejected (stalls are overworld-only)                                                                 |
| 11.5 | A tries to **close stall with full inventory** | Rejected: "Make room first" (items can't return)                                                     |
| 11.6 | Multiple buyers browse the same stall          | Both see items. First buyer purchases → second buyer's view should update                            |


---

## 12. Cross-Feature Interactions


| #    | Step                                                | Expected                                                                    |
| ---- | --------------------------------------------------- | --------------------------------------------------------------------------- |
| 12.1 | A has stall open. B right-clicks A                  | Context menu shows "Browse Shop" but **not** "Trade" (or Trade is rejected) |
| 12.2 | A is in a trade. A presses Y to open stall          | Stall should not open while trading                                         |
| 12.3 | A opens stall with items, B buys one, then A closes | A gets remaining items back + gold earned. B has purchased item             |
| 12.4 | A is sitting in a chair, tries to open stall        | Verify behavior (should it be allowed?)                                     |


---

## Smoke Test Checklist (Quick Pass)

- Trade request → accept → offer items → both accept → complete
- Trade cancel works
- Walking away cancels trade
- Press Y → add items → Open Shop → can't move
- Other player right-clicks → Browse Shop → Buy item
- Close Shop → items return → can move again
- No crashes throughout

