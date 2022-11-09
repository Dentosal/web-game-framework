use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rules::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(Uuid);
impl PlayerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(Uuid);
impl GameId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    /// Claim an old identity, so that in case of disconnect we can rejoin games
    Identify { secret: [u8; 16] },
    /// Associate a nickname with this identity
    SetName(String),
    /// Join a game by id, creating it if it doesn't already exist.
    /// Use None to generate a random id.
    JoinGame(Option<GameId>),
    /// Leave the current game
    LeaveGame,
    /// Kick a player from a game (only available as the game leader)
    KickPlayer(PlayerId),
    /// Give game leader status to another player
    PromoteLeader(PlayerId),
    /// Start the game (only available as the game leader)
    StartGame,
    /// Play your turn
    Play(ActionPrivate),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub you: PlayerId,
    pub leader: PlayerId,
    pub player_names: HashMap<PlayerId, String>,
    /// Ordered for the turn order
    pub players: Vec<PlayerId>,
    pub state: Option<GamePublic>,
    pub player_state: Option<PlayerPrivate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerMessage {
    ServerInfo { version: String },
    Success,
    Error { message: String },
    NewIdentity { secret: [u8; 16] },
    GameInfo { id: GameId, info: GameInfo },
}
