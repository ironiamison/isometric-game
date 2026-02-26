# Spectator Login Screen Design

## Overview

Replace the static starry background on the login screen with a live view of the game world. The client connects as a read-only spectator on launch, streams world data, and renders the full game world behind the login form with a slow cinematic camera drift. On login, the spectator connection upgrades to a full authenticated session seamlessly — no reconnect needed.

## High-Level Flow

1. **On launch**: Client immediately opens a spectator WebSocket connection to the server (no auth needed). The starry background renders as usual.
2. **Spectator connects**: Server begins streaming chunk data, NPC positions, and player positions around the spawn point. Once enough chunks load, the starry sky crossfades (~1-2s) into the live world view.
3. **Server unreachable**: The starry sky simply stays. No error, no disruption.
4. **On login**: Client authenticates via existing HTTP auth endpoints, then sends the auth token over the existing spectator WebSocket as an "upgrade" message. Server validates, creates the player entity, and transitions the connection to a full authenticated session.
5. **Gameplay begins**: Camera smoothly pans from cinematic drift to centering on the player's character. Already-loaded chunks and entity data carry over — no loading screen needed.

## Server-Side Spectator Protocol

### Spectator Connection

- Server accepts WebSocket connections without auth at `/spectate`
- No player entity is created — spectators do not exist in the game world
- Server sends `StateSync` and chunk data for the spawn area on a one-way stream
- **The server completely ignores all incoming messages from spectators** — no parsing, no dispatch, no processing. The WebSocket read side is a black hole until session upgrade.
- Spectators cannot move, chat, interact, request specific chunks, or influence the game state in any way

### Session Upgrade

- Client authenticates via existing HTTP login/register endpoints
- On success, client sends the auth token over the spectator WebSocket as a single designated "upgrade" message — the **only** message type the server will accept from a spectator
- Server validates the token, creates the player entity, and transitions the connection to a full authenticated session
- If the token is invalid, server sends an error and connection stays in spectator mode
- From this point, the connection behaves identically to a normal authenticated session

### Hardening

- Spectator sessions are rate-limited (cap on max concurrent spectators)
- No data is sent that isn't already visible to any logged-in player (public world state only)
- Spectators receive no private data — no inventories, no quest states, no chat messages

## Client-Side Rendering & Camera

### Camera Drift

- A `SpectatorCamera` drives a slow cinematic path around the spawn area
- Defines waypoints near spawn in a gentle loop or figure-eight pattern
- Camera smoothly interpolates between waypoints at a slow, constant speed
- Loops seamlessly
- Stays at default zoom level

### Rendering in Spectator Mode

- Reuses the existing world renderer with minimal modifications
- Same tile rendering, NPC/player rendering, depth sorting
- **Chat is skipped** — no chat log, no chat bubbles above players
- All UI panels skipped (inventory, skills, quest log, etc.)
- Login form renders on top of the world view with semi-transparent backdrop
- No mouse interaction with the world — clicks only hit the login UI

### Crossfade Transition

- When spectator connection loads enough chunks (`is_world_ready()` equivalent), starry background alpha fades out over ~1-2 seconds
- World view fades in underneath
- If connection drops, keep showing whatever was last rendered — no jarring snap back to stars

### Login-to-Gameplay Transition

- Camera smoothly pans from current drift position to player's spawn location
- Normal gameplay rendering takes over seamlessly

## State Machine Changes

### New AppState Structure

`AppState::Login` reworked to hold both the `LoginScreen` and an optional spectator connection + game state:

- On launch: `LoginScreen` starts with starry background, spectator connection begins in background
- When spectator connects and world is ready: renderer switches from starry sky to world view
- Login form stays on top throughout

### Spectator Game State

A lightweight version of `GameState` holding:

- `chunk_manager` — for streamed tile data
- `players` / `npcs` — for entity rendering
- `spectator_camera` — the drifting camera
- No inventory, no local player, no UI state, no input handling

### Transition to Playing

- Existing spectator `chunk_manager` (with already-loaded chunks) carries over into full `GameState`
- Player/NPC data carries over too — no re-download needed
- `SpectatorCamera` hands off to normal `Camera`, which animates to player's position
- `AppState` transitions to `Playing` with the upgraded connection and reused world data
- Result: instant transition into gameplay — world is already loaded and rendering
