use std::collections::HashSet;

use wgfw_protocol::PlayerId;

pub trait Game: Send + Sync {
    /// Extract public game state that is visible to all players
    fn public_state(&self, common: &GameCommon) -> serde_json::Value;
    /// Extract private game state that is only visible to a single player
    fn state_for_player(&self, common: &GameCommon, player: PlayerId) -> serde_json::Value;

    /// Does the game accept new players at the moment?
    fn can_join(&self, _common: &GameCommon) -> bool {
        true // Default to always allowing joins
    }

    /// Does the game accept reconnecting players at the moment?
    fn can_reconnect(&self, _common: &GameCommon) -> bool {
        true // Default to always allowing reconnects
    }

    fn on_disconnect(&mut self, _common: &GameCommon, _player: PlayerId) {}
    fn on_reconnect(&mut self, _common: &GameCommon, _player: PlayerId) {}
    fn on_join(&mut self, _common: &GameCommon, _player: PlayerId) {}
    fn on_leave(&mut self, _common: &GameCommon, _player: PlayerId) {}
    fn on_kick(&mut self, _common: &GameCommon, _player: PlayerId) {}

    fn on_message_from(
        &mut self,
        common: &GameCommon,
        player: PlayerId,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value>;
}

#[derive(Debug)]
pub struct GameCommon {
    pub leader: PlayerId,
    pub players: HashSet<PlayerId>,
}

pub struct Lobby {
    /// Common state for all game types
    pub common: GameCommon,
    /// State specific to the current game type
    pub state: Box<dyn Game>,
}
impl Lobby {
    pub fn try_remove_player(&mut self, player: &PlayerId) -> bool {
        let removed = self.common.players.remove(player);
        if !removed {
            return false;
        }

        // Assign a "random" leader
        if *player == self.common.leader {
            let new_leader = self
                .common
                .players
                .iter()
                .next()
                .copied()
                .unwrap_or_else(PlayerId::new);
            self.common.leader = new_leader;
        }

        true
    }

    pub fn public_state(&self) -> serde_json::Value {
        self.state.public_state(&self.common)
    }

    pub fn state_for_player(&self, player: PlayerId) -> serde_json::Value {
        self.state.state_for_player(&self.common, player)
    }

    pub fn can_join(&self) -> bool {
        self.state.can_join(&self.common)
    }

    pub fn can_reconnect(&self) -> bool {
        self.state.can_reconnect(&self.common)
    }

    pub fn on_disconnect(&mut self, player: PlayerId) {
        self.state.on_disconnect(&self.common, player);
    }

    pub fn on_reconnect(&mut self, player: PlayerId) {
        self.state.on_reconnect(&self.common, player);
    }

    pub fn on_join(&mut self, player: PlayerId) {
        self.state.on_join(&self.common, player);
    }

    pub fn on_leave(&mut self, player: PlayerId) {
        self.state.on_leave(&self.common, player);
    }

    pub fn on_kick(&mut self, player: PlayerId) {
        self.state.on_kick(&self.common, player);
    }

    pub fn on_message_from(
        &mut self,
        player: PlayerId,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        self.state.on_message_from(&self.common, player, message)
    }
}
