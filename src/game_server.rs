use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use futures::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use warp::ws::{Message, WebSocket};

use skullgame::protocol::{self, ClientMessage, GameId, GameInfo, PlayerId, ServerMessage};
use skullgame::rules::{GamePrivate, GamePublic, PlayerPrivate};
use skullgame::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(Uuid);
impl ClientId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug)]
enum Event {
    Connected {
        client_id: ClientId,
        tx: SplitSink<WebSocket, Message>,
    },
    Disconnected {
        client_id: ClientId,
    },
    Message {
        client_id: ClientId,
        payload: protocol::ClientMessage,
    },
    InvalidMessage {
        client_id: ClientId,
        error: serde_json::Error,
    },
}

pub fn spawn() -> (JoinHandle<()>, ServerRemote) {
    let (event_tx, event_rx) = mpsc::channel(64);

    let jh = tokio::spawn(async {
        GameServer::new().run(event_rx).await;
    });

    (jh, ServerRemote { event_tx })
}

struct Player {
    name: String,
    tx: SplitSink<WebSocket, Message>,
}

pub enum GameState {
    Waiting,
    Running {
        player_order: Vec<PlayerId>,
        state: GamePrivate,
    },
}

pub struct Game {
    pub leader: PlayerId,
    pub players: HashSet<PlayerId>,
    pub state: GameState,
}
impl Game {
    pub fn state_for(&self, player: &PlayerId) -> Option<(GamePublic, PlayerPrivate)> {
        match &self.state {
            GameState::Waiting => None,
            GameState::Running {
                player_order,
                state,
            } => {
                let i = player_order.iter().position(|p| p == player)?;
                Some(state.for_player(i))
            }
        }
    }

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
                .unwrap_or_else(|| PlayerId::new());
            self.leader = new_leader;
        }

        true
    }

    pub fn start(&mut self) {
        let mut player_order: Vec<_> = self.players.iter().copied().collect();
        let mut rng = thread_rng();
        player_order.shuffle(&mut rng);
        let player_count = player_order.len();
        self.state = GameState::Running {
            player_order,
            state: GamePrivate::new(player_count),
        }
    }
}

struct GameServer {
    clients: HashMap<ClientId, PlayerId>,
    players: HashMap<PlayerId, Player>,
    games: HashMap<GameId, Game>,
}
impl GameServer {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            players: HashMap::new(),
            games: HashMap::new(),
        }
    }

    async fn broadcast_game_state(&mut self, game_id: GameId) {
        let game = self.games.get(&game_id).unwrap();

        for player_id in game.players.iter() {
            let (state, player_state) = match game.state_for(&player_id) {
                Some((a, b)) => (Some(a), Some(b)),
                None => (None, None),
            };

            let players: Vec<PlayerId> = match &game.state {
                GameState::Waiting => {
                    let mut players: Vec<_> = game.players.iter().copied().collect();
                    players.sort();
                    players
                }
                GameState::Running { player_order, .. } => {
                    let mut not_in_game: Vec<PlayerId> = game
                        .players
                        .iter()
                        .copied()
                        .filter(|p| !player_order.contains(&p))
                        .collect();
                    not_in_game.sort();
                    player_order.iter().copied().chain(not_in_game).collect()
                }
            };

            let response = serde_json::to_string(&protocol::ServerMessage::GameInfo {
                id: game_id,
                info: GameInfo {
                    you: *player_id,
                    leader: game.leader,
                    player_names: game
                        .players
                        .iter()
                        .map(|p_id| {
                            let p = self.players.get(p_id).unwrap();
                            (*p_id, p.name.clone())
                        })
                        .collect(),
                    players,
                    state,
                    player_state,
                },
            })
            .unwrap();

            let player = self.players.get_mut(&player_id).unwrap();
            let _ = player.tx.send(Message::text(response)).await;
        }
    }

    async fn run(mut self, mut event_rx: mpsc::Receiver<Event>) {
        while let Some(event) = event_rx.recv().await {
            log::debug!("Event: {:?}", event);

            match event {
                Event::Connected { client_id, mut tx } => {
                    let response = serde_json::to_string(&ServerMessage::ServerInfo {
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                    })
                    .unwrap();
                    tx.send(Message::text(response)).await.unwrap();

                    let player_id = PlayerId::new();

                    let old = self.clients.insert(client_id, player_id);
                    debug_assert!(old.is_none(), "The client id should never conflict");
                    let old = self.players.insert(
                        player_id,
                        Player {
                            name: "Anonymous".to_owned(),
                            tx,
                        },
                    );
                    debug_assert!(old.is_none(), "The client id should never conflict");
                }
                Event::Disconnected { client_id } => {
                    let player_id = self.clients.remove(&client_id).unwrap();
                    let _ = self.players.remove(&player_id).unwrap();
                    let affected_games: HashSet<GameId> = self
                        .games
                        .iter_mut()
                        .filter_map(|(game_id, game)| {
                            if game.try_remove_player(&player_id) {
                                Some(*game_id)
                            } else {
                                None
                            }
                        })
                        .collect();

                    for game_id in affected_games {
                        self.broadcast_game_state(game_id).await;
                    }
                }
                Event::InvalidMessage { client_id, error } => {
                    let player_id = self.clients.get(&client_id).unwrap();
                    let player = self.players.get_mut(&player_id).unwrap();

                    let response = serde_json::to_string(&protocol::ServerMessage::Error {
                        message: format!("{}", error),
                    })
                    .unwrap();
                    player.tx.send(Message::text(response)).await.unwrap();
                }
                Event::Message { client_id, payload } => match payload {
                    ClientMessage::Identify { .. } => todo!("Identify"),
                    ClientMessage::SetName(name) => {
                        let player_id = self.clients.get(&client_id).unwrap();
                        self.players.get_mut(&player_id).unwrap().name = name;
                    }
                    ClientMessage::JoinGame(game_id) => {
                        let player_id = *self.clients.get(&client_id).unwrap();

                        let game_id = game_id.unwrap_or(GameId::new());
                        let game = self.games.entry(game_id).or_insert_with(|| Game {
                            leader: player_id,
                            players: HashSet::new(),
                            state: GameState::Waiting,
                        });
                        game.players.insert(player_id);
                        self.broadcast_game_state(game_id).await;
                    }
                    ClientMessage::LeaveGame => todo!("LeaveGame"),
                    ClientMessage::KickPlayer(_) => todo!("KickPlayer"),
                    ClientMessage::PromoteLeader(_) => todo!("PromoteLeader"),
                    ClientMessage::StartGame => {
                        let player_id = self.clients.get(&client_id).unwrap();
                        let Some((game_id, game)) = self.games.iter_mut().find(|(_, g)| g.players.contains(player_id)) else {
                            log::warn!("Player trying to start a game, but not in any lobby");
                            continue;
                        };
                        if game.players.len() < 2 {
                            log::warn!("Trying to start a game with not enough players");
                            continue;
                        }
                        if !matches!(game.state, GameState::Waiting) {
                            log::warn!("Trying to start a game that has already started");
                            continue;
                        }
                        game.start();
                        let game_id = *game_id;
                        drop(game);
                        self.broadcast_game_state(game_id).await;
                    }
                    ClientMessage::Play(_) => todo!("Play"),
                },
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
        let client_id = ClientId::new();
        log::debug!(
            "New connection from {:?} with client id {:?}",
            self.peer_addr,
            client_id
        );

        let (tx, mut rx) = websocket.split();

        self.server
            .event_tx
            .send(Event::Connected { client_id, tx })
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
                match serde_json::from_str::<protocol::ClientMessage>(&msg) {
                    Ok(payload) => self
                        .server
                        .event_tx
                        .send(Event::Message { client_id, payload })
                        .await
                        .unwrap(),
                    Err(error) => self
                        .server
                        .event_tx
                        .send(Event::InvalidMessage { client_id, error })
                        .await
                        .unwrap(),
                }
            }
        }

        self.server
            .event_tx
            .send(Event::Disconnected { client_id })
            .await
            .unwrap();
    }
}
