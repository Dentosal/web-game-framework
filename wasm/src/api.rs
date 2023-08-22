//! User-visible API of the websocket-wrapper

use wasm_bindgen::prelude::*;
use wgfw_protocol::{ClientMessageData, ReplyMessage};

use crate::WgfwEvents;

/// Callback field setters
#[wasm_bindgen]
impl WgfwEvents {
    #[wasm_bindgen(setter)]
    pub fn set_onready(&self, value: js_sys::Function) {
        *self.onready.lock().unwrap() = Some(value);
    }

    #[wasm_bindgen(setter)]
    pub fn set_onupdate(&self, value: js_sys::Function) {
        *self.onupdate.lock().unwrap() = Some(value);
    }

    #[wasm_bindgen(setter)]
    pub fn set_onerror(&self, value: js_sys::Function) {
        *self.onerror.lock().unwrap() = Some(value);
    }
}

macro_rules! server_msg_c1 {
    ($typename:ident, $($an:ident),+) => {
        ClientMessageData::$typename($(
            serde_wasm_bindgen::from_value($an).expect("Failed to convert")
        ),*)
    };
    ($typename:ident) => {
        ClientMessageData::$typename
    };
}

macro_rules! server_msg {
    ($typename:ident, $replyname:ident, $name:ident $(, $an:ident : $at:ident)*) => {
        #[wasm_bindgen]
        impl WgfwEvents {
            #[wasm_bindgen]
            pub async fn $name(&self, $($an: JsValue),*) -> Result<JsValue, String> {
                let (tx, rx) = futures::channel::oneshot::channel::<ReplyMessage>();
                self.send_message(
                    server_msg_c1!($typename $(,$an)*),
                    Box::new(move |data| {
                        tx.send(data).unwrap();
                    }),
                );

                rx.await
                    .map(|value| match value {
                        ReplyMessage::$replyname(value) => {
                            Ok(serde_wasm_bindgen::to_value(&value).unwrap())
                        }
                        ReplyMessage::Error(err) => Err(format!("{:?}", err)),
                        _ => panic!("Unexpected reply"),
                    })
                    .unwrap()
            }
        }
    };
}

// Messages to server
server_msg!(GameModes, GameModes, game_modes);
server_msg!(JoinedGames, JoinedGames, joined_games);
server_msg!(CreateGame, GameCreated, create_game, game_type: String);
server_msg!(JoinGame, JoinedToGame, join_game, game_id: GameId);
server_msg!(Inner, Inner, inner, game_id: GameId, inner: JsValue);
