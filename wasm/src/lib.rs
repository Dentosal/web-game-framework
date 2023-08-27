mod api;
mod storage;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use gloo_utils::format::JsValueSerdeExt;
use js_sys::Array;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use wgfw_protocol::{
    ClientMessageData, ErrorReply, Identity, MessageId, ReplyMessage, ServerMessage,
    ServerSentMessage,
};

macro_rules! console_log {
    ($($t:tt)*) => (crate::log(&format_args!($($t)*).to_string()))
}

pub(crate) use console_log;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Clone)]
#[wasm_bindgen]
pub struct WgfwEvents {
    ws: WebSocket,
    reply_callbacks: Arc<Mutex<HashMap<MessageId, Box<dyn FnOnce(ReplyMessage)>>>>,
    /// Ready and identified
    onready: Arc<Mutex<Option<js_sys::Function>>>,
    /// Received server-initiated message
    onupdate: Arc<Mutex<Option<js_sys::Function>>>,
    /// Socket closed unexpectedly, matches both onerror and onclose callbacks
    onerror: Arc<Mutex<Option<js_sys::Function>>>,
}

#[wasm_bindgen]
impl WgfwEvents {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WgfwEvents {
        // Connect to an echo server
        let self_ = Self {
            ws: WebSocket::new("ws://localhost:3030/ws").expect("failed to open ws"),
            reply_callbacks: Arc::default(),
            onready: Arc::default(),
            onerror: Arc::default(),
            onupdate: Arc::default(),
        };
        self_.start_websocket().expect("error!");
        self_
    }

    fn send_message(&self, msg: ClientMessageData, callback: Box<dyn FnOnce(ReplyMessage)>) {
        let msg = msg.finalize();
        self.ws
            .send_with_str(serde_json::to_string(&msg).unwrap().as_str())
            .expect("Send error");
        self.reply_callbacks
            .lock()
            .unwrap()
            .insert(msg.id, callback);
    }

    /// Must be called when connecting for the first time
    fn make_new_identity(&self) {
        let cloned_self = self.clone();
        self.send_message(
            ClientMessageData::NewIdentity,
            Box::new(move |reply| match reply {
                ReplyMessage::Identity(identity) => {
                    console_log!("Got new identity: {:?}", identity);
                    cloned_self.identify_done(identity);
                }
                _ => {
                    console_log!("Unexpected reply: {:?}", reply);
                }
            }),
        );
    }

    /// Must be called when connecting for the first time
    fn identify(&self) {
        // Get identity from local storage, if any. If not, request a new one.
        let cloned_self = self.clone();
        if let Some(old_identity) = storage::get_typed::<Identity>("wgfw_identity") {
            console_log!("Found old identity: {:?}", old_identity);
            self.send_message(
                ClientMessageData::Identify(old_identity),
                Box::new(move |reply| match reply {
                    ReplyMessage::Identity(identity) => {
                        console_log!("Restored old identity: {:?}", identity);
                        cloned_self.identify_done(identity);
                    }
                    ReplyMessage::Error(ErrorReply::InvalidReconnectionSecret) => {
                        // The server has restarted, or the identity has been purged.
                        // Request a new identity.
                        console_log!("Could not restore old identity, requesting new one");
                        cloned_self.make_new_identity();
                    }
                    _ => {
                        console_log!("Unexpected reply: {:?}", reply);
                    }
                }),
            );
        } else {
            console_log!("No old identity found, requesting new one");
            self.make_new_identity();
        }
    }

    /// called by identify() when it's ready
    fn identify_done(&self, identity: Identity) {
        storage::set_typed("wgfw_identity", &identity);
        if let Some(onready) = self.onready.lock().unwrap().as_ref() {
            onready
                .call1(
                    &JsValue::NULL,
                    &serde_wasm_bindgen::to_value(&identity.player_id).unwrap(),
                )
                .expect("onready errored");
        } else {
            console_log!("No onready callback");
        }
    }

    fn start_websocket(&self) -> Result<(), JsValue> {
        // Callback: onmessage
        let cloned_self = self.clone();
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let txt = txt.as_string().unwrap();
                let msg: ServerMessage =
                    serde_json::from_str(txt.as_str()).expect("Invalid message from server");
                console_log!("message event: {:?}", msg);
                match msg {
                    ServerMessage::ServerSent(msg) => match msg {
                        ServerSentMessage::Error { message } => {
                            console_log!("Error: {:?}", message);
                        }
                        ServerSentMessage::GameInfo {
                            id,
                            leader,
                            players,
                            public_state,
                            private_state,
                        } => {
                            if let Some(onupdate) = cloned_self.onupdate.lock().unwrap().as_ref() {
                                onupdate
                                    .apply(
                                        &JsValue::NULL,
                                        &Array::from_iter(
                                            [
                                                JsValue::from_serde(&id).unwrap(),
                                                JsValue::from_serde(&leader).unwrap(),
                                                JsValue::from_serde(&players).unwrap(),
                                                JsValue::from_serde(&public_state).unwrap(),
                                                JsValue::from_serde(&private_state).unwrap(),
                                            ]
                                            .into_iter(),
                                        ),
                                    )
                                    .unwrap();
                            }
                        }
                    },
                    ServerMessage::ReplyTo(message_id, msg) => {
                        let callback = {
                            let mut callbacks = cloned_self.reply_callbacks.lock().unwrap();
                            callbacks.remove(&message_id).unwrap()
                        };
                        callback(msg);
                    }
                }
            }
        });
        self.ws
            .set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Callback: onopen
        let cloned_self = self.clone();
        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            cloned_self.identify();
        });
        self.ws
            .set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        // Callback: onerror
        let cloned_self = self.clone();
        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            console_log!("error event: {:?}", e);
            if let Some(onerror) = cloned_self.onerror.lock().unwrap().as_ref() {
                onerror.call1(&JsValue::NULL, &e).unwrap();
            } else {
                console_log!("No onerror callback! Error {e:?}");
            }
        });
        self.ws
            .set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        // Callback: onclose
        let cloned_self = self.clone();
        let onclose_callback = Closure::<dyn FnMut()>::new(move || {
            console_log!("socket closed");
            if let Some(onerror) = cloned_self.onerror.lock().unwrap().as_ref() {
                onerror.call0(&JsValue::NULL).expect("onerror errored");
            } else {
                console_log!("No onerror callback! Socket closed");
            }
        });
        self.ws
            .set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        Ok(())
    }
}

#[wasm_bindgen(start)]
fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    // let window = web_sys::window().expect("no global `window` exists");
    Ok(())
}
