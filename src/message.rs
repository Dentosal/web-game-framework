use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    game_state::GameId,
    player::{PlayerId, ReconnectionSecret},
};

/// Message id, used to match replies to requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct MessageId(Uuid);
impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    pub id: MessageId,
    pub data: ClientMessageData,
}

/// Message from client to server
#[derive(Debug, Deserialize, Serialize)]
pub enum ClientMessageData {
    CreateGame(String),
    JoinGame(GameId),
    LeaveGame(GameId),
    KickPlayer(GameId, PlayerId),
    PromoteLeader(GameId, PlayerId),

    /// When reconnecting, identify as a player
    Identify(PlayerId, ReconnectionSecret),

    /// Game-specific message
    Inner(GameId, serde_json::Value),
}
impl ClientMessageData {
    pub fn finalize(self) -> ClientMessage {
        ClientMessage {
            id: MessageId::new(),
            data: self,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ServerMessage {
    /// Server-initiated message
    ServerSent(ServerSentMessage),
    /// Reply to a client message
    ReplyTo(MessageId, ReplyMessage),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ServerSentMessage {
    Initialized {
        server_version: String,
        player_id: PlayerId,
        reconnection_secret: ReconnectionSecret,
    },
    Error {
        message: String,
    },
    GameInfo {
        id: GameId,
        leader: PlayerId,
        players: Vec<PlayerId>,
        public_state: serde_json::Value,
        private_state: serde_json::Value,
    },
}
impl ServerSentMessage {
    pub fn finalize(self) -> ServerMessage {
        ServerMessage::ServerSent(self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ReplyMessage {
    Ok,
    GameCreated(GameId),
    JoinedToGame(GameId),
    Error(ErrorReply),
}

impl ReplyMessage {
    pub fn finalize(self, reply_to: MessageId) -> ServerMessage {
        ServerMessage::ReplyTo(reply_to, self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ErrorReply {
    InvalidGameFormat,
    NoSuchGameLobby,
    NotInThatGame,
}
