use std::collections::HashSet;

use tokio::time::Instant;
use uuid::Uuid;

use wgfw_protocol::{GameId, PlayerId};

use crate::{event_queue::EventQueue, game_server::PublishGameState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(Uuid);
impl EventId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Updates needed after processing a message or event
#[must_use]
pub struct Updates {
    /// Broadcast new state to all players
    pub state_changed: bool,
    /// Schedule timer-delayed events
    pub events: Vec<(Instant, EventId)>,
}
impl Updates {
    pub const CHANGED: Self = Self::new(true);
    pub const NONE: Self = Self::new(false);

    pub const fn new(state_changed: bool) -> Self {
        Self {
            state_changed,
            events: Vec::new(),
        }
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.state_changed |= other.state_changed;
        self.events.extend(other.events);
        self
    }

    pub fn add_timeout(&mut self, at: Instant) -> EventId {
        let id = EventId::new();
        self.events.push((at, id));
        id
    }

    pub fn always_publish(mut self) -> Self {
        self.state_changed = true;
        self
    }

    pub(crate) fn apply(
        self,
        game_id: GameId,
        publish: &mut PublishGameState,
        scheduled: &mut EventQueue<(GameId, EventId)>,
    ) {
        if self.state_changed {
            publish.add_all(game_id);
        }

        self.apply_schedule(game_id, scheduled);
    }

    /// Returns true if the state changed and needs broadcasting
    pub(crate) fn apply_schedule(
        self,
        game_id: GameId,
        scheduled: &mut EventQueue<(GameId, EventId)>,
    ) -> bool {
        for (at, event_id) in self.events {
            scheduled.add((game_id, event_id), at);
        }

        self.state_changed
    }
}

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

    fn on_disconnect(&mut self, _common: &GameCommon, _player: PlayerId) -> Updates {
        Updates::NONE
    }
    fn on_reconnect(&mut self, _common: &GameCommon, _player: PlayerId) -> Updates {
        Updates::NONE
    }
    fn on_join(&mut self, _common: &GameCommon, _player: PlayerId) -> Updates {
        Updates::NONE
    }
    fn on_leave(&mut self, _common: &GameCommon, _player: PlayerId) -> Updates {
        Updates::NONE
    }
    fn on_kick(&mut self, _common: &GameCommon, _player: PlayerId) -> Updates {
        Updates::NONE
    }

    fn on_event(&mut self, _common: &GameCommon, _id: EventId) -> Updates {
        panic!("No event handler defined, but an event was scheduled");
    }

    fn on_message_from(
        &mut self,
        common: &GameCommon,
        player: PlayerId,
        message: serde_json::Value,
    ) -> (Updates, Result<serde_json::Value, serde_json::Value>);
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

    pub fn on_disconnect(&mut self, player: PlayerId) -> Updates {
        self.state.on_disconnect(&self.common, player)
    }

    pub fn on_reconnect(&mut self, player: PlayerId) -> Updates {
        self.state.on_reconnect(&self.common, player)
    }

    pub fn on_join(&mut self, player: PlayerId) -> Updates {
        self.state.on_join(&self.common, player)
    }

    pub fn on_leave(&mut self, player: PlayerId) -> Updates {
        self.state.on_leave(&self.common, player)
    }

    pub fn on_kick(&mut self, player: PlayerId) -> Updates {
        self.state.on_kick(&self.common, player)
    }

    pub fn on_event(&mut self, id: EventId) -> Updates {
        self.state.on_event(&self.common, id)
    }

    pub fn on_message_from(
        &mut self,
        player: PlayerId,
        message: serde_json::Value,
    ) -> (Updates, Result<serde_json::Value, serde_json::Value>) {
        self.state.on_message_from(&self.common, player, message)
    }
}
