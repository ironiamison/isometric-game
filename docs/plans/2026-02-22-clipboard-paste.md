# Clipboard Paste (Ctrl+V) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Support Ctrl+V / Cmd+V paste into the chat input bar on all platforms (native, WASM, Android).

**Architecture:** Miniquad already has `clipboard_get()` built in for native (macOS/Linux/Windows) and WASM (via paste event listener in mq_js_bundle.js). We just need to detect Ctrl+V/Cmd+V in the chat input handler and insert the clipboard text at the cursor position.

**Tech Stack:** Existing `macroquad::miniquad::window::clipboard_get()` — no new dependencies.

---

### Task 1: Add Ctrl+V paste handling to chat input

**Files:**
- Modify: `client/src/input/handler.rs:5303` (after Home/End handling, before Backspace)

**Step 1: Add paste detection and insertion**

Insert this block after the Home/End key handling (line 5302) and before the Backspace handling (line 5304):

```rust
            // Paste from clipboard (Ctrl+V / Cmd+V)
            let ctrl_held = is_key_down(KeyCode::LeftControl)
                || is_key_down(KeyCode::RightControl)
                || is_key_down(KeyCode::LeftSuper)
                || is_key_down(KeyCode::RightSuper);
            if ctrl_held && is_key_pressed(KeyCode::V) {
                if let Some(text) = macroquad::miniquad::window::clipboard_get() {
                    for c in text.chars() {
                        if state.ui_state.chat_input.chars().count() >= 200 {
                            break;
                        }
                        if c.is_control() {
                            continue;
                        }
                        let byte_idx = char_to_byte_index(
                            &state.ui_state.chat_input,
                            state.ui_state.chat_cursor,
                        );
                        state.ui_state.chat_input.insert(byte_idx, c);
                        state.ui_state.chat_cursor += 1;
                    }
                }
                // Drain char queue to prevent 'v' from leaking through
                while get_char_pressed().is_some() {}
            }
```

**Step 2: Build and verify**

Run: `cargo build -p isometric-client 2>&1 | tail -5`
Expected: Compiles successfully.

Run: `cargo build -p isometric-client --target wasm32-unknown-unknown --profile release-wasm 2>&1 | tail -5`
Expected: WASM build compiles successfully.

**Step 3: Manual test**

- Native: Open chat, copy text from elsewhere, Ctrl+V → text appears at cursor
- WASM: Same test in browser
- Verify: pasting respects 200 char limit, control chars (newlines) are stripped, cursor position is correct after paste

**Step 4: Commit**

```bash
git add client/src/input/handler.rs
git commit -m "feat: support Ctrl+V paste in chat input"
```
