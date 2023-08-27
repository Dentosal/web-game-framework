use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::{iter, mem};

use futures::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};

use wgfw_protocol::{
    ClientMessage, ClientMessageData, ErrorReply, GameId, Identity, PlayerId, ReconnectionSecret,
    ReplyMessage, ServerSentMessage,
};

use crate::game_registry::GameRegistry;
use crate::game_state::{GameCommon, Lobby};

/// Browser session
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(Uuid);
impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

struct Player {
    tx: SplitSink<WebSocket, Message>,
    identified: bool,
}

#[derive(Debug)]
struct Event {
    client: ConnectionId,
    data: EventData,
}

#[derive(Debug)]
enum EventData {
    Connected(SplitSink<WebSocket, Message>),
    Disconnected,
    Message(ClientMessage),
    InvalidMessage(serde_json::Error),
}
impl EventData {
    fn finalize(self, client: ConnectionId) -> Event {
        Event { client, data: self }
    }
}

pub fn spawn(registry: GameRegistry) -> (JoinHandle<()>, ServerRemote) {
    let (event_tx, event_rx) = mpsc::channel(64);

    let jh = tokio::spawn(async {
        GameServer {
            secret: orion::auth::SecretKey::generate(32).expect("Unable to generate secret key"),
            clients: HashMap::new(),
            players: HashMap::new(),
            games: HashMap::new(),
            registry,
        }
        .run(event_rx)
        .await;
    });

    (jh, ServerRemote { event_tx })
}

struct GameServer {
    /// Secret key used for signing reconnection tokens
    secret: orion::auth::SecretKey,
    /// Currently connected ws clients -> PlayerId mapping
    clients: HashMap<ConnectionId, PlayerId>,
    /// PlayerId -> ws client mapping. This doesn't exist for disconnected players.
    players: HashMap<PlayerId, Player>,
    /// GameId -> Game Lobby mapping
    games: HashMap<GameId, Lobby>,
    /// Game type registry
    registry: GameRegistry,
}
impl GameServer {
    async fn send_state_to_player(&mut self, game_id: GameId, player_id: PlayerId) {
        let game = self.games.get(&game_id).unwrap();
        if !game.common.players.contains(&player_id) {
            return;
        }

        let public_state = game.public_state();
        let private_state = game.state_for_player(player_id);

        let mut players: Vec<_> = game.common.players.iter().copied().collect();
        players.sort();

        let message = ServerSentMessage::GameInfo {
            id: game_id,
            leader: game.common.leader,
            players,
            public_state,
            private_state,
        }
        .finalize();

        let response = serde_json::to_string(&message).unwrap();
        if let Some(player) = self.players.get_mut(&player_id) {
            let _ = player.tx.send(Message::text(response)).await;
        }
    }

    async fn broadcast_game_state(&mut self, game_id: GameId) {
        let game = self.games.get(&game_id).unwrap();
        let players: Vec<PlayerId> = game.common.players.iter().copied().collect();

        for player_id in players {
            self.send_state_to_player(game_id, player_id).await;
        }
    }

    async fn run(mut self, mut event_rx: mpsc::Receiver<Event>) {
        while let Some(event) = event_rx.recv().await {
            log::debug!("Event: {:?}", event);

            match event.data {
                EventData::Connected(tx) => {
                    let player_id = PlayerId::new();

                    let old = self.clients.insert(event.client, player_id);
                    debug_assert!(old.is_none(), "The client id should never conflict");
                    let old = self.players.insert(
                        player_id,
                        Player {
                            tx,
                            identified: false,
                        },
                    );
                    debug_assert!(old.is_none(), "The client id should never conflict");
                }
                EventData::Disconnected => {
                    let player_id = self.clients.remove(&event.client).unwrap();
                    let _ = self.players.remove(&player_id).unwrap();
                    let affected_games: HashSet<GameId> = self
                        .games
                        .iter()
                        .filter_map(|(game_id, game)| {
                            if game.common.players.contains(&player_id) {
                                Some(*game_id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    for game_id in affected_games {
                        self.games
                            .get_mut(&game_id)
                            .unwrap()
                            .on_disconnect(player_id);
                        self.broadcast_game_state(game_id).await;
                    }
                }
                EventData::InvalidMessage(error) => {
                    let player_id = self.clients.get(&event.client).unwrap();
                    let player = self.players.get_mut(player_id).unwrap();

                    let response = serde_json::to_string(
                        &ServerSentMessage::Error {
                            message: format!("{}", error),
                        }
                        .finalize(),
                    )
                    .unwrap();
                    player.tx.send(Message::text(response)).await.unwrap();
                }
                EventData::Message(cmsg) => self.process_client_message(event.client, cmsg).await,
            };
        }
    }

    async fn process_client_message(&mut self, client: ConnectionId, msg: ClientMessage) {
        let mut publish = PublishGameState::default();

        let ClientMessage { id: msgid, data } = msg;
        let mut player_id = *self.clients.get(&client).unwrap();

        let is_identified = self.players.get(&player_id).unwrap().identified;
        let attempts_to_identify = matches!(
            data,
            ClientMessageData::NewIdentity | ClientMessageData::Identify(..)
        );

        let response: ReplyMessage = if is_identified && attempts_to_identify {
            ReplyMessage::Error(ErrorReply::AlreadyIdentified)
        } else if !is_identified && !attempts_to_identify {
            ReplyMessage::Error(ErrorReply::MustIdentifyFirst)
        } else {
            match data {
                ClientMessageData::NewIdentity => {
                    self.players.get_mut(&player_id).unwrap().identified = true;
                    ReplyMessage::Identity(Identity {
                        player_id,
                        reconnection_secret: ReconnectionSecret::for_player(
                            &self.secret,
                            player_id,
                        ),
                    })
                }
                ClientMessageData::Identify(identity) => {
                    if identity.verify(&self.secret) {
                        let old_player_id = mem::replace(&mut player_id, identity.player_id);
                        self.clients.insert(client, identity.player_id);
                        let mut old_entry = self.players.remove(&old_player_id).unwrap();
                        old_entry.identified = true;
                        self.players.insert(identity.player_id, old_entry);

                        // Notify running games about reconnection
                        let affected_games: HashSet<GameId> = self
                            .games
                            .iter()
                            .filter_map(|(game_id, game)| {
                                if game.common.players.contains(&player_id) {
                                    Some(*game_id)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        for game_id in affected_games {
                            self.games
                                .get_mut(&game_id)
                                .unwrap()
                                .on_reconnect(player_id);
                            publish.add_all(game_id);
                        }
                        ReplyMessage::Identity(identity)
                    } else {
                        ReplyMessage::Error(ErrorReply::InvalidReconnectionSecret)
                    }
                }
                ClientMessageData::GameModes => {
                    ReplyMessage::GameModes(self.registry.games.keys().cloned().collect())
                }
                ClientMessageData::JoinedGames => {
                    let games: Vec<_> = self
                        .games
                        .iter()
                        .filter_map(|(game_id, game)| {
                            if game.common.players.contains(&player_id) {
                                Some(*game_id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    // Send game state to player
                    for game_id in games.iter() {
                        publish.add(*game_id, player_id);
                    }

                    ReplyMessage::JoinedGames(games)
                }
                ClientMessageData::CreateGame(game_type) => {
                    let game_id = GameId::new();
                    if let Some(constructor) = self.registry.games.get(&game_type) {
                        let state = constructor();
                        self.games.insert(
                            game_id,
                            Lobby {
                                common: GameCommon {
                                    leader: player_id,
                                    players: iter::once(player_id).collect(),
                                },
                                state,
                            },
                        );
                        self.games.get_mut(&game_id).unwrap().on_join(player_id);
                        publish.add_all(game_id);
                        ReplyMessage::GameCreated(game_id)
                    } else {
                        ReplyMessage::Error(ErrorReply::InvalidGameFormat)
                    }
                }
                ClientMessageData::JoinGame(game_id) => {
                    let player_id = *self.clients.get(&client).unwrap();

                    if let Some(game) = self.games.get_mut(&game_id) {
                        game.common.players.insert(player_id);
                        self.games.get_mut(&game_id).unwrap().on_join(player_id);
                        publish.add_all(game_id);
                        ReplyMessage::JoinedToGame(game_id)
                    } else {
                        ReplyMessage::Error(ErrorReply::NoSuchGameLobby)
                    }
                }
                ClientMessageData::LeaveGame(game_id) => {
                    let player_id = *self.clients.get(&client).unwrap();

                    if let Some(game) = self.games.get_mut(&game_id) {
                        game.common.players.remove(&player_id);
                        self.games.get_mut(&game_id).unwrap().on_leave(player_id);
                        publish.add_all(game_id);
                        ReplyMessage::Ok
                    } else {
                        ReplyMessage::Error(ErrorReply::NoSuchGameLobby)
                    }
                }
                ClientMessageData::KickPlayer(_, _) => todo!("KickPlayer"),
                ClientMessageData::PromoteLeader(_, _) => todo!("PromoteLeader"),
                ClientMessageData::Inner(game_id, inner_data) => {
                    if let Some(game) = self.games.get_mut(&game_id) {
                        if game.common.players.contains(&player_id) {
                            let reply = game.on_message_from(player_id, inner_data);
                            publish.add_all(game_id); // TODO: only when needed
                            match reply {
                                Ok(value) => ReplyMessage::Inner(value),
                                Err(err) => ReplyMessage::Error(ErrorReply::Inner(err)),
                            }
                        } else {
                            ReplyMessage::Error(ErrorReply::NotInThatGame)
                        }
                    } else {
                        ReplyMessage::Error(ErrorReply::NoSuchGameLobby)
                    }
                }
            }
        };

        let reply = response.finalize(msgid);
        self.players
            .get_mut(&player_id)
            .unwrap()
            .tx
            .send(Message::text(serde_json::to_string(&reply).unwrap()))
            .await
            .unwrap();

        publish.run(self).await;
    }
}

/// Keeps track of which game states need sending to which players
#[derive(Debug, Default)]
struct PublishGameState {
    /// `None` as value means all players
    games: HashMap<GameId, Option<HashSet<PlayerId>>>,
}
impl PublishGameState {
    pub fn add(&mut self, game_id: GameId, player_id: PlayerId) {
        match self.games.entry(game_id) {
            Entry::Occupied(mut entry) => {
                if let Some(players) = entry.get_mut() {
                    players.insert(player_id);
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(Some(iter::once(player_id).collect()));
            }
        }
    }

    pub fn add_all(&mut self, game_id: GameId) {
        self.games.insert(game_id, None);
    }

    pub async fn run(self, server: &mut GameServer) {
        for (game_id, players) in self.games {
            if let Some(players) = players {
                for player_id in players {
                    server.send_state_to_player(game_id, player_id).await;
                }
            } else {
                server.broadcast_game_state(game_id).await;
            }
        }
    }
}

#[derive(Clone)]
pub struct ServerRemote {
    event_tx: mpsc::Sender<Event>,
}
impl ServerRemote {
    pub fn make_client_handle(&self, peer_addr: SocketAddr) -> ClientHandle {
        ClientHandle {
            server: self.clone(),
            peer_addr,
        }
    }
}

#[derive(Clone)]
pub struct ClientHandle {
    server: ServerRemote,
    peer_addr: SocketAddr,
}

impl ClientHandle {
    pub async fn handle_ws_client(self, websocket: WebSocket) {
        let client_id = ConnectionId::new();
        log::debug!(
            "New connection from {:?} with client id {:?}",
            self.peer_addr,
            client_id
        );

        let (tx, mut rx) = websocket.split();

        self.server
            .event_tx
            .send(EventData::Connected(tx).finalize(client_id))
            .await
            .unwrap();

        while let Some(body) = rx.next().await {
            let message = match body {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("error reading message on websocket: {}", e);
                    break;
                }
            };

            // Skip non-text messages
            if let Ok(msg) = message.to_str() {
                match serde_json::from_str::<ClientMessage>(msg) {
                    Ok(payload) => self
                        .server
                        .event_tx
                        .send(EventData::Message(payload).finalize(client_id))
                        .await
                        .unwrap(),
                    Err(error) => self
                        .server
                        .event_tx
                        .send(EventData::InvalidMessage(error).finalize(client_id))
                        .await
                        .unwrap(),
                }
            }
        }

        self.server
            .event_tx
            .send(EventData::Disconnected.finalize(client_id))
            .await
            .unwrap();
    }
}
