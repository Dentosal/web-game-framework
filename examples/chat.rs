#![deny(unused_must_use)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use warp::Filter;

use wgfw::{game_state::Game, Builder, PlayerId};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Chat {
    pub title: String,
    pub messages: Vec<ChatMessage>,
    pub nicknames: HashMap<PlayerId, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    pub sender: PlayerId,
    pub text: String,
    pub formatting: Option<String>,
}

/// Message sent by the client
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UserMessage {
    /// Send a chat message
    Chat(String),
    /// Change the title of the chat
    Title(String),
    /// Change nickname in this chat
    Nick(String),
}

impl Game for Chat {
    fn public_state(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }

    fn state_for_player(&self, _player: PlayerId) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn on_disconnect(&mut self, player: PlayerId) {
        self.messages.push(ChatMessage {
            sender: player,
            text: format!("disconnected"),
            formatting: Some("server_issued".to_owned()),
        });
    }

    fn on_reconnect(&mut self, player: PlayerId) {
        self.messages.push(ChatMessage {
            sender: player,
            text: format!("reconnected"),
            formatting: Some("server_issued".to_owned()),
        });
    }

    fn on_join(&mut self, player: PlayerId) {
        self.messages.push(ChatMessage {
            sender: player,
            text: format!("joined"),
            formatting: Some("server_issued".to_owned()),
        });
    }

    fn on_leave(&mut self, player: PlayerId) {
        self.messages.push(ChatMessage {
            sender: player,
            text: format!("left"),
            formatting: Some("server_issued".to_owned()),
        });
    }

    fn on_kick(&mut self, player: PlayerId) {
        self.messages.push(ChatMessage {
            sender: player,
            text: format!("kicked out"),
            formatting: Some("server_issued".to_owned()),
        });
    }

    fn on_message_from(
        &mut self,
        player: PlayerId,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        if let Ok(msg) = serde_json::from_value(message) {
            match msg {
                UserMessage::Chat(text) => {
                    self.messages.push(ChatMessage {
                        sender: player,
                        text: text.to_string(),
                        formatting: None,
                    });
                }
                UserMessage::Title(title) => {
                    self.title = title;
                }
                UserMessage::Nick(name) => {
                    self.nicknames.insert(player, name);
                }
            }
            Ok(().into())
        } else {
            Err("Invalid message!!".into())
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let index = warp::get()
        .and(warp::path::end())
        .and(warp::fs::file("./examples/chat_static/index.html"));

    let favicon = warp::get()
        .and(warp::path("favicon.ico"))
        .and(warp::fs::file("./examples/chat_static/images/favicon.ico"));

    let static_files = warp::path("static").and(warp::fs::dir("./examples/chat_static/"));

    let (game_server, ws) = Builder::new().register::<Chat>("chat").spawn();

    let web_server =
        warp::serve(index.or(favicon).or(static_files).or(ws)).run(([127, 0, 0, 1], 3030));

    let ((), game_server_result) = tokio::join!(web_server, game_server);
    game_server_result.expect("Game server panicked");
}
