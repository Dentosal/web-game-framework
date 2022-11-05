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
pub enum ClientMessage {
    /// Claim an old identity, so that in case of disconnect we can rejoin games
    Identify { secret: [u8; 16] },
    /// Associate a nickname with this identity
    SetName(String),
    /// Creates a game and joins it as the leader
    CreateGame,
    /// Join a game by id
    JoinGame(GameId),
    /// Leave the current game
    LeaveGame,
    /// Kick a player from a game (only available as the game leader)
    KickPlayer(PlayerId),
    /// Start the game (only available as the game leader)
    StartGame(GameId),
    /// Play your turn
    Play(ActionPrivate),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub leader: PlayerId,
    pub players: Vec<PlayerInfo>,
    pub state: Option<GamePublic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Success,
    Error,
    NewIdentity { sercret: [u8; 16] },
    GameCreated(GameId),
    GameInfo(GameInfo),
}
