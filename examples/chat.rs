#![deny(unused_must_use)]

use serde::{Deserialize, Serialize};
use warp::Filter;

use wgfw::{game_state::Game, Builder, PlayerId};

#[derive(Debug, Default, Serialize, Deserialize)]
struct Chat {
    pub title: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    pub sender: PlayerId,
    pub text: String,
}

impl Game for Chat {
    fn public_state(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }

    fn state_for_player(&self, _player: PlayerId) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn on_disconnect(&mut self, player: PlayerId) {
        todo!()
    }

    fn on_reconnect(&mut self, player: PlayerId) {
        todo!()
    }

    fn on_kicked(&mut self, player: PlayerId) {
        todo!()
    }

    fn on_message_from(&mut self, player: PlayerId, message: serde_json::Value) {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let index = warp::get()
        .and(warp::path::end())
        .and(warp::fs::file("./static/index.html"));

    let favicon = warp::get()
        .and(warp::path("favicon.ico"))
        .and(warp::fs::file("./static/images/favicon.ico"));

    let static_files = warp::path("static").and(warp::fs::dir("./static/"));

    let (game_server, ws) = Builder::new().register::<Chat>("chat").spawn();

    let web_server =
        warp::serve(index.or(favicon).or(static_files).or(ws)).run(([127, 0, 0, 1], 3030));

    let ((), game_server_result) = tokio::join!(web_server, game_server);
    game_server_result.expect("Game server panicked");
}
