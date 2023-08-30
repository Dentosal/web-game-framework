//! User-visible API of the websocket-wrapper

use gloo_utils::format::JsValueSerdeExt;

use wasm_bindgen::prelude::*;
use wgfw_protocol::{ClientMessageData, ErrorReply, ReplyMessage};

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

macro_rules! server_msg_c2 {
    ($ra:ident) => {
        serde_wasm_bindgen::to_value(&$ra).unwrap()
    };
    () => {
        JsValue::null()
    };
}

macro_rules! server_msg {
    ($typename:ident, $replyname:ident $(($ra:ident))?, $name:ident $(, $an:ident : $at:ident)*) => {
        #[wasm_bindgen]
        impl WgfwEvents {
            #[wasm_bindgen]
            pub async fn $name(&self, $($an: JsValue),*) -> Result<JsValue, JsValue> {
                let (tx, rx) = futures::channel::oneshot::channel::<ReplyMessage>();
                crate::console_log!("Sending message {:?} {:?}", stringify!($name), ($(&$an),*));
                self.send_message(
                    server_msg_c1!($typename $(,$an)*),
                    Box::new(move |data| {
                        tx.send(data).unwrap();
                    }),
                );

                rx.await
                    .map(|value| match value {
                        ReplyMessage::$replyname $(($ra))? => Ok(server_msg_c2!($($ra)?)),
                        ReplyMessage::Error(err) => Err(match err {
                            ErrorReply::Inner(inner) => JsValue::from_serde(&inner).unwrap(),
                            other => JsValue::from_str(&format!("{:?}", other)),
                        }),
                        _ => panic!("Unexpected reply"),
                    })
                    .unwrap()
            }
        }
    };
}

// Messages to server
server_msg!(GameModes, GameModes(v), game_modes);
server_msg!(JoinedGames, JoinedGames(v), joined_games);
server_msg!(CreateGame, GameCreated(v), create_game, game_type: String);
server_msg!(JoinGame, JoinedToGame(v), join_game, game_id: GameId);
server_msg!(LeaveGame, Ok, leave_game, game_id: GameId);
server_msg!(Inner, Inner(v), inner, game_id: GameId, inner: JsValue);
