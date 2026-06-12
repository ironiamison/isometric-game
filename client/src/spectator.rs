use crate::config::WS_URL;
use crate::game::{self, GameState};
use crate::network::{self, NetworkClient};

/// Spectator state for login/character select screens — streams the world behind the UI.
///
/// Opens a read-only connection to the server's `/spectate` endpoint and renders the
/// live world as a backdrop. When the player starts the game, the same connection is
/// upgraded in place to a full player session (see `NetworkClient::send_spectator_upgrade`),
/// reusing the chunks already loaded here for a seamless transition.
pub(crate) struct SpectatorState {
    pub game_state: GameState,
    pub network: NetworkClient,
    pub camera: game::SpectatorCamera,
    pub crossfade_alpha: f32, // 0.0 = stars fully visible, 1.0 = world fully visible
    pub world_ready: bool,
}

impl SpectatorState {
    pub fn new() -> Self {
        let mut game_state = GameState::new();
        game_state.spectator_mode = true;
        let network = NetworkClient::new_spectator(WS_URL);
        Self {
            game_state,
            network,
            camera: game::SpectatorCamera::new(),
            crossfade_alpha: 0.0,
            world_ready: false,
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Poll network messages into game state
        self.network.poll(&mut self.game_state);

        // Update spectator camera
        let (cx, cy) = self.camera.update(dt);
        self.game_state.camera.x = cx;
        self.game_state.camera.y = cy;
        self.game_state.camera.zoom = 1.0;
        self.game_state.camera.initialized = true;

        // Request chunks around camera position (spectator has no local player)
        let chunks_to_request = self.game_state.chunk_manager.update_player_position(cx, cy);
        for coord in chunks_to_request {
            self.network
                .send(&network::messages::ClientMessage::RequestChunk {
                    chunk_x: coord.x,
                    chunk_y: coord.y,
                });
        }
        self.game_state.chunk_manager.unload_distant_chunks();

        // Check world readiness and drive crossfade
        if !self.world_ready && self.game_state.is_world_ready() {
            self.world_ready = true;
        }

        if self.world_ready {
            // Fade in over ~1.5 seconds
            self.crossfade_alpha = (self.crossfade_alpha + dt / 1.5).min(1.0);
        }
    }
}
