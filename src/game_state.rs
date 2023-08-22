use std::collections::HashSet;

use wgfw_protocol::PlayerId;

pub trait Game: Send + Sync {
    /// Extract public game state that is visible to all players
    fn public_state(&self) -> serde_json::Value;
    /// Extract private game state that is only visible to a single player
    fn state_for_player(&self, player: PlayerId) -> serde_json::Value;

    /// Does the game accept new players at the moment?
    fn can_join(&self, _players: &HashSet<PlayerId>) -> bool {
        true // Default to always allowing joins
    }

    /// Does the game accept reconnecting players at the moment?
    fn can_reconnect(&self, _players: &HashSet<PlayerId>) -> bool {
        true // Default to always allowing reconnects
    }

    fn on_disconnect(&mut self, player: PlayerId);
    fn on_reconnect(&mut self, player: PlayerId);
    fn on_kicked(&mut self, player: PlayerId);
    fn on_message_from(
        &mut self,
        player: PlayerId,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value>;
}

pub struct Lobby {
    pub leader: PlayerId,
    pub players: HashSet<PlayerId>,
    pub state: Box<dyn Game>,
}
impl Lobby {
    pub fn try_remove_player(&mut self, player: &PlayerId) -> bool {
        let removed = self.players.remove(player);
        if !removed {
            return false;
        }

        // Assign a "random" leader
        if *player == self.leader {
            let new_leader = self
                .players
                .iter()
                .next()
                .copied()
                .unwrap_or_else(PlayerId::new);
            self.leader = new_leader;
        }

        true
    }
}
