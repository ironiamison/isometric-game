# Mobile Chat UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a mobile-friendly fullscreen chat panel with tabs (Local, Global, System), triggered by a dedicated chat button in the top-left corner.

**Architecture:** Repurpose existing chat state fields where possible. Add a new `chat_panel.rs` render module following the same patterns as `menu.rs` (fullscreen overlay with `draw_panel_frame`). The chat button is a small icon rendered above the quest tracker. Native keyboard handles text input on Android.

**Tech Stack:** Rust, macroquad, existing UI framework (UiLayout, UiElementId, draw_panel_frame)

---

### Task 1: Add UiElementId variants and UiState fields

**Files:**
- Modify: `client/src/ui/layout.rs:60-80` (add new enum variants)
- Modify: `client/src/game/state.rs:401-540` (add new fields + defaults)

**Step 1: Add UiElementId variants**

In `client/src/ui/layout.rs`, add after the `GoldDropCancel` variant (line 79):

```rust
    // Chat Panel
    ChatButton,
    ChatTabLocal,
    ChatTabGlobal,
    ChatTabSystem,
    ChatInputField,
    ChatSendButton,
    ChatPanelBackground,
```

**Step 2: Add UiState fields**

In `client/src/game/state.rs`, add after `chat_log_visible` (line 458):

```rust
    // Mobile chat panel
    pub chat_panel_open: bool,
    pub chat_active_tab: ChatChannel,
```

**Step 3: Add defaults**

In the `Default` impl for `UiState`, add after `chat_log_visible` defaults (around line 533):

```rust
            chat_panel_open: false,
            chat_active_tab: ChatChannel::Local,
```

**Step 4: Add Clone/Copy derive to ChatChannel**

`ChatChannel` needs `Clone, Copy, PartialEq` if not already derived. Check `client/src/game/state.rs:195` and add if missing.

**Step 5: Commit**

```bash
git add client/src/ui/layout.rs client/src/game/state.rs
git commit -m "feat: add chat panel state and UI element IDs"
```

---

### Task 2: Create chat panel renderer

**Files:**
- Create: `client/src/render/ui/chat_panel.rs`
- Modify: `client/src/render/ui/mod.rs` (add module)

**Step 1: Add module declaration**

In `client/src/render/ui/mod.rs`, add:

```rust
pub mod chat_panel;
```

**Step 2: Create chat_panel.rs**

Create `client/src/render/ui/chat_panel.rs` with the fullscreen chat panel rendering. Follow the `menu.rs` pattern:

```rust
//! Mobile chat panel rendering - fullscreen overlay with tabs

use macroquad::prelude::*;
use crate::game::{GameState, ChatChannel};
use crate::ui::{UiElementId, UiLayout};
use crate::util::virtual_screen_size;
use super::super::Renderer;
use super::common::*;

impl Renderer {
    /// Render the fullscreen chat panel overlay
    pub(crate) fn render_chat_panel(&self, state: &GameState, hovered: &Option<UiElementId>, layout: &mut UiLayout) {
        if !state.ui_state.chat_panel_open {
            return;
        }

        let (sw, sh) = virtual_screen_size();

        // Semi-transparent overlay (blocks game interaction)
        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
        layout.add(UiElementId::ChatPanelBackground, macroquad::prelude::Rect::new(0.0, 0.0, sw, sh));

        // Panel dimensions - nearly fullscreen with small margin
        let margin = 10.0;
        let panel_x = margin;
        let panel_y = margin;
        let panel_w = sw - margin * 2.0;
        let panel_h = sh - margin * 2.0;

        // Panel frame
        self.draw_panel_frame(panel_x, panel_y, panel_w, panel_h);

        // === TAB BAR ===
        let tab_y = panel_y + FRAME_THICKNESS;
        let tab_w = (panel_w - FRAME_THICKNESS * 2.0) / 3.0;
        let tab_h = TAB_HEIGHT;
        let tab_x_start = panel_x + FRAME_THICKNESS;

        let tabs = [
            (UiElementId::ChatTabLocal, "Local", ChatChannel::Local),
            (UiElementId::ChatTabGlobal, "Global", ChatChannel::Global),
            (UiElementId::ChatTabSystem, "System", ChatChannel::System),
        ];

        for (i, (id, label, channel)) in tabs.iter().enumerate() {
            let tx = tab_x_start + i as f32 * tab_w;
            let is_active = std::mem::discriminant(&state.ui_state.chat_active_tab) == std::mem::discriminant(channel);
            let is_hovered = hovered.as_ref() == Some(id);

            let bg = if is_active {
                HEADER_BG
            } else if is_hovered {
                SLOT_HOVER_BG
            } else {
                PANEL_BG_DARK
            };

            draw_rectangle(tx, tab_y, tab_w, tab_h, bg);
            draw_rectangle_lines(tx, tab_y, tab_w, tab_h, 1.0, HEADER_BORDER);

            if is_active {
                // Gold underline for active tab
                draw_rectangle(tx + 2.0, tab_y + tab_h - 2.0, tab_w - 4.0, 2.0, FRAME_ACCENT);
            }

            let text_w = self.measure_text_sharp(label, TAB_FONT_SIZE).width;
            self.draw_text_sharp(label, (tx + (tab_w - text_w) / 2.0).floor(),
                               (tab_y + tab_h / 2.0 + 5.0).floor(), TAB_FONT_SIZE,
                               if is_active { TEXT_TITLE } else { TEXT_DIM });

            layout.add(id.clone(), macroquad::prelude::Rect::new(tx, tab_y, tab_w, tab_h));
        }

        // === MESSAGE LIST ===
        let messages_y = tab_y + tab_h + 4.0;
        let input_bar_h = 48.0;
        let is_system_tab = matches!(state.ui_state.chat_active_tab, ChatChannel::System);
        let messages_h = if is_system_tab {
            panel_y + panel_h - FRAME_THICKNESS - messages_y
        } else {
            panel_y + panel_h - FRAME_THICKNESS - input_bar_h - 4.0 - messages_y
        };
        let messages_x = panel_x + FRAME_THICKNESS + 8.0;
        let messages_w = panel_w - FRAME_THICKNESS * 2.0 - 16.0;

        // Message area background
        draw_rectangle(panel_x + FRAME_THICKNESS, messages_y,
                      panel_w - FRAME_THICKNESS * 2.0, messages_h, PANEL_BG_DARK);

        // Filter and render messages
        let font_size = 16.0;
        let line_height = 20.0;
        let max_lines = (messages_h / line_height) as usize;

        let filtered: Vec<_> = state.ui_state.chat_messages.iter()
            .filter(|m| std::mem::discriminant(&m.channel) == std::mem::discriminant(&state.ui_state.chat_active_tab))
            .collect();

        // Render from bottom up, newest messages at bottom
        let mut y = messages_y + messages_h - line_height;
        let mut lines_drawn = 0;

        for msg in filtered.iter().rev() {
            if lines_drawn >= max_lines {
                break;
            }

            let (color, text) = match msg.channel {
                ChatChannel::Local => (WHITE, format!("{}: {}", msg.sender_name, msg.text)),
                ChatChannel::Global => (SKYBLUE, format!("[G] {}: {}", msg.sender_name, msg.text)),
                ChatChannel::System => (Color::from_rgba(255, 220, 100, 255), format!("{} {}", msg.sender_name, msg.text)),
            };

            let wrapped = self.wrap_text(&text, messages_w, font_size);
            for line in wrapped.iter().rev() {
                if lines_drawn >= max_lines || y < messages_y {
                    break;
                }
                self.draw_text_sharp(line, messages_x, y, font_size, color);
                y -= line_height;
                lines_drawn += 1;
            }
        }

        // === INPUT BAR (hidden on System tab) ===
        if !is_system_tab {
            let input_y = panel_y + panel_h - FRAME_THICKNESS - input_bar_h;
            let send_btn_w = 60.0;
            let input_w = panel_w - FRAME_THICKNESS * 2.0 - send_btn_w - 12.0;
            let input_x = panel_x + FRAME_THICKNESS + 4.0;

            // Input field background
            draw_rectangle(input_x, input_y, input_w, input_bar_h, SLOT_BG_EMPTY);
            draw_rectangle_lines(input_x, input_y, input_w, input_bar_h, 1.0, SLOT_BORDER);

            // Input text
            let text_y = input_y + input_bar_h / 2.0 + 5.0;
            let display_text = if state.ui_state.chat_input.is_empty() {
                "Tap to chat..."
            } else {
                &state.ui_state.chat_input
            };
            let text_color = if state.ui_state.chat_input.is_empty() { TEXT_DIM } else { TEXT_NORMAL };
            self.draw_text_sharp(display_text, input_x + 8.0, text_y, font_size, text_color);

            layout.add(UiElementId::ChatInputField, macroquad::prelude::Rect::new(input_x, input_y, input_w, input_bar_h));

            // Send button
            let send_x = input_x + input_w + 8.0;
            let is_send_hovered = hovered.as_ref() == Some(&UiElementId::ChatSendButton);
            let send_bg = if is_send_hovered { SLOT_HOVER_BG } else { HEADER_BG };
            draw_rectangle(send_x, input_y, send_btn_w, input_bar_h, send_bg);
            draw_rectangle_lines(send_x, input_y, send_btn_w, input_bar_h, 1.0, FRAME_MID);

            let send_label = "Send";
            let send_w = self.measure_text_sharp(send_label, font_size).width;
            self.draw_text_sharp(send_label, (send_x + (send_btn_w - send_w) / 2.0).floor(),
                                text_y, font_size, TEXT_TITLE);

            layout.add(UiElementId::ChatSendButton, macroquad::prelude::Rect::new(send_x, input_y, send_btn_w, input_bar_h));
        }
    }
}
```

**Step 3: Commit**

```bash
git add client/src/render/ui/chat_panel.rs client/src/render/ui/mod.rs
git commit -m "feat: add chat panel renderer with tabs and input bar"
```

---

### Task 3: Render chat button and hook panel into render pipeline

**Files:**
- Modify: `client/src/render/renderer.rs:3615-3710` (render_interactive_ui)
- Modify: `client/src/render/ui/quest.rs:164-203` (shift quest tracker down)

**Step 1: Render chat button in render_interactive_ui**

In `render_interactive_ui` (renderer.rs), add before the quest tracker call (before line 3652):

```rust
        // Chat button (top-left, above quest tracker) - mobile only
        #[cfg(target_os = "android")]
        {
            let chat_btn_x = 10.0;
            let chat_btn_y = 10.0;
            if let Some(tex) = &self.chat_small_icon {
                let btn_size = 32.0;
                draw_texture_ex(tex, chat_btn_x, chat_btn_y, WHITE, DrawTextureParams {
                    dest_size: Some(Vec2::new(btn_size, btn_size)),
                    ..Default::default()
                });
                layout.add(UiElementId::ChatButton, macroquad::prelude::Rect::new(chat_btn_x, chat_btn_y, btn_size, btn_size));
            }
        }
```

**Step 2: Render chat panel on top of everything**

In `render_interactive_ui`, add after the escape menu rendering (after line 3707, before `layout` return):

```rust
        // Chat panel (fullscreen overlay, on top of everything)
        self.render_chat_panel(state, hovered, &mut layout);
```

**Step 3: Shift quest tracker down on Android**

In `client/src/render/ui/quest.rs`, modify the `tracker_y` calculation (line 170) to account for the chat button:

```rust
        let tracker_y = if state.debug_mode {
            460.0
        } else {
            #[cfg(target_os = "android")]
            { 50.0 } // Below chat button (10 + 32 + 8)
            #[cfg(not(target_os = "android"))]
            { 20.0 }
        };
```

**Step 4: Commit**

```bash
git add client/src/render/renderer.rs client/src/render/ui/quest.rs
git commit -m "feat: render chat button and panel in UI pipeline"
```

---

### Task 4: Add input handling for chat panel elements

**Files:**
- Modify: `client/src/input/handler.rs:650-660` (near MenuButtonSocial handler)

**Step 1: Add ChatButton click handler**

In handler.rs, add a new match arm for `ChatButton` in the UI element click handling section (near the MenuButton handlers around line 650):

```rust
                    UiElementId::ChatButton => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_panel_open = !state.ui_state.chat_panel_open;
                        if state.ui_state.chat_panel_open {
                            state.ui_state.chat_active_tab = ChatChannel::Local;
                            // Close other panels
                            state.ui_state.inventory_open = false;
                            state.ui_state.character_panel_open = false;
                            state.ui_state.skills_open = false;
                            state.ui_state.social_open = false;
                        }
                    }
```

**Step 2: Add tab click handlers**

```rust
                    UiElementId::ChatTabLocal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Local;
                    }
                    UiElementId::ChatTabGlobal => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::Global;
                    }
                    UiElementId::ChatTabSystem => {
                        audio.play_sfx("enter");
                        state.ui_state.chat_active_tab = ChatChannel::System;
                    }
```

**Step 3: Add send button handler**

```rust
                    UiElementId::ChatSendButton => {
                        let text = state.ui_state.chat_input.trim().to_string();
                        if !text.is_empty() {
                            audio.play_sfx("send_message");
                            commands.push(InputCommand::Chat { text });
                        }
                        state.ui_state.chat_input.clear();
                        state.ui_state.chat_cursor = 0;
                    }
```

**Step 4: Add input field tap handler (triggers native keyboard)**

```rust
                    UiElementId::ChatInputField => {
                        // On Android, this should trigger the native keyboard
                        // The existing chat_open flow handles keyboard input
                        state.ui_state.chat_open = true;
                    }
```

**Step 5: Add ChatPanelBackground handler (consume taps, prevent game interaction)**

```rust
                    UiElementId::ChatPanelBackground => {
                        // Consume tap - don't let it pass through to game world
                    }
```

**Step 6: Block game input when chat panel is open**

Near the top of the input handler's main update function, add an early return when the chat panel is open (similar to how other panels block input). Find where chat_open is checked (line 1385) and add before it:

```rust
        // Block game-world input when chat panel is open (mobile)
        if state.ui_state.chat_panel_open {
            // Only process UI element clicks (handled above), not game world input
            return commands;
        }
```

**Step 7: Commit**

```bash
git add client/src/input/handler.rs
git commit -m "feat: add input handling for chat panel buttons and tabs"
```

---

### Task 5: Wire up native keyboard and integrate with existing chat flow

**Files:**
- Modify: `client/src/input/handler.rs` (chat_open section around line 1385)

**Step 1: Modify existing chat_open handler for panel integration**

When `chat_open` is true and `chat_panel_open` is also true, Enter should send the message but keep the panel open (instead of closing chat). Modify the Enter handler (around line 1410):

```rust
            if is_key_pressed(KeyCode::Enter) {
                let text = state.ui_state.chat_input.trim().to_string();
                if !text.is_empty() {
                    audio.play_sfx("send_message");
                    commands.push(InputCommand::Chat { text });
                }
                state.ui_state.chat_input.clear();
                state.ui_state.chat_cursor = 0;
                state.ui_state.chat_scroll_offset = 0;
                // If chat panel is open, keep it open but close keyboard input
                if state.ui_state.chat_panel_open {
                    state.ui_state.chat_open = false;
                } else {
                    state.ui_state.chat_open = false;
                }
                return commands;
            }
```

**Step 2: Escape in chat panel closes panel instead of just chat input**

Modify the Escape handler (around line 1401):

```rust
            if is_key_pressed(KeyCode::Escape) {
                state.ui_state.chat_open = false;
                state.ui_state.chat_input.clear();
                state.ui_state.chat_cursor = 0;
                state.ui_state.chat_scroll_offset = 0;
                if state.ui_state.chat_panel_open {
                    state.ui_state.chat_panel_open = false;
                }
                return commands;
            }
```

**Step 3: Commit**

```bash
git add client/src/input/handler.rs
git commit -m "feat: integrate native keyboard with chat panel"
```

---

### Task 6: Test and polish

**Files:**
- Various minor adjustments as needed

**Step 1: Build for Android and verify**

```bash
cd client && ./scripts/run-android.sh
```

**Step 2: Verify checklist**
- [ ] Chat button visible top-left, quest tracker below it
- [ ] Tapping chat button opens fullscreen panel
- [ ] Three tabs work, Local is default
- [ ] Messages display filtered by tab with correct colors
- [ ] Input bar visible on Local/Global tabs, hidden on System tab
- [ ] Tapping input triggers native keyboard
- [ ] Send button dispatches message
- [ ] Tapping chat button again closes panel
- [ ] Game still runs behind overlay
- [ ] Game touch input blocked while panel is open

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat: mobile chat UI with fullscreen panel and channel tabs"
```
