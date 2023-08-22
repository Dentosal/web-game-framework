//! User-visible API of the websocket-wrapper

use wasm_bindgen::prelude::*;
use wgfw_protocol::{ClientMessageData, ReplyMessage};

use crate::WgfwEvents;

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

    #[wasm_bindgen]
    pub async fn create_game(&self, game_type: String) -> Result<JsValue, String> {
        let (tx, rx) = futures::channel::oneshot::channel::<ReplyMessage>();
        self.send_message(
            ClientMessageData::CreateGame(game_type),
            Box::new(move |data| {
                tx.send(data).unwrap();
            }),
        );

        rx.await
            .map(|value| match value {
                ReplyMessage::GameCreated(id) => Ok(serde_wasm_bindgen::to_value(&id).unwrap()),
                ReplyMessage::Error(err) => Err(format!("{:?}", err)),
                _ => panic!("Unexpected reply"),
            })
            .unwrap()
    }
}
