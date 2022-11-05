use std::collections::HashMap;
use std::net::SocketAddr;

use futures::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use warp::ws::{Message, WebSocket};

use skullgame::protocol::{self, ClientMessage, GameId, GameInfo, PlayerId, PlayerInfo};
use skullgame::rules::GamePrivate;
use skullgame::Uuid;

#[derive(Debug)]
enum Event {
    Connected {
        client_id: Uuid,
        tx: SplitSink<WebSocket, Message>,
    },
    Disconnected {
        client_id: Uuid,
    },
    Message {
        client_id: Uuid,
        payload: protocol::ClientMessage,
    },
}

pub fn spawn() -> (JoinHandle<()>, ServerRemote) {
    let (event_tx, event_rx) = mpsc::channel(64);

    let jh = tokio::spawn(async {
        game_server(event_rx).await;
    });

    (jh, ServerRemote { event_tx })
}

struct Player {
    info: PlayerInfo,
    tx: SplitSink<WebSocket, Message>,
}

pub struct Game {
    pub leader: PlayerId,
    pub players: Vec<PlayerId>,
    pub state: Option<GamePrivate>,
}

async fn game_server(mut event_rx: mpsc::Receiver<Event>) {
    let mut players: HashMap<Uuid, Player> = HashMap::new();
    let mut games: HashMap<GameId, Game> = HashMap::new();

    while let Some(event) = event_rx.recv().await {
        log::debug!("Event: {:?}", event);

        match event {
            Event::Connected { client_id, tx } => {
                let old = players.insert(
                    client_id,
                    Player {
                        info: PlayerInfo {
                            id: PlayerId::new(),
                            name: "Anonymous".to_owned(),
                        },
                        tx,
                    },
                );
                debug_assert!(old.is_none(), "The client id should never conflict");
            }
            Event::Disconnected { client_id } => todo!(),
            Event::Message { client_id, payload } => match payload {
                ClientMessage::Identify { .. } => todo!("Identify"),
                ClientMessage::SetName(name) => {
                    players.get_mut(&client_id).unwrap().info.name = name;
                }
                ClientMessage::CreateGame => {
                    let player = players.get_mut(&client_id).unwrap();

                    let game_id = GameId::new();
                    games.insert(
                        game_id,
                        Game {
                            leader: player.info.id,
                            players: vec![player.info.id],
                            state: None,
                        },
                    );
                    let response =
                        serde_json::to_string(&protocol::ServerMessage::GameCreated(game_id))
                            .unwrap();
                    player.tx.send(Message::text(response)).await.unwrap();
                }
                ClientMessage::JoinGame(_) => todo!("JoinGame"),
                ClientMessage::LeaveGame => todo!("LeaveGame"),
                ClientMessage::KickPlayer(_) => todo!("KickPlayer"),
                ClientMessage::StartGame(_) => todo!("StartGame"),
                ClientMessage::Play(_) => todo!("Play"),
            },
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
        let client_id = Uuid::new_v4();
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
                let payload: protocol::ClientMessage = serde_json::from_str(&msg).unwrap();
                self.server
                    .event_tx
                    .send(Event::Message { client_id, payload })
                    .await
                    .unwrap();
            }
        }

        self.server
            .event_tx
            .send(Event::Disconnected { client_id })
            .await
            .unwrap();
    }
}
