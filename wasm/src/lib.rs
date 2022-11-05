use skullgame::protocol::ServerMessage;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use skullgame::protocol::ClientMessage;

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: String);

    type Error;

    #[wasm_bindgen(constructor)]
    fn new() -> Error;

    #[wasm_bindgen(structural, method, getter)]
    fn stack(error: &Error) -> String;
}

pub fn panic_hook(info: &std::panic::PanicInfo) {
    let mut msg = info.to_string();

    msg.push_str("\n\nStack:\n\n");
    let e = Error::new();
    let stack = e.stack();
    msg.push_str(&stack);
    msg.push_str("\n\n");
    error(msg);
}

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(panic_hook));

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    let elem = document
        .get_element_by_id("loading-banner")
        .expect("no #loading-banner");
    elem.set_class_name("nodisplay");

    // let val = document
    //     .get_element_by_id("all-players")
    //     .expect("no #all-players");
    // val.set_inner_html("Hello from Rust!");

    let elem = document
        .create_element("div")?;
    elem.set_class_name("start-screen");
    elem.set_inner_html(include_str!("html/start.html"));
    body.append_child(&elem)?;

    socket();

    Ok(())
}

fn socket() {
    let ws = WebSocket::new("ws://localhost:3030/ws").expect("Connection error");

    // Setup msg callback
    let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
        if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
            let msg: ServerMessage = serde_json::from_str(&txt.as_string().unwrap()).unwrap();
            console_log!("message event, received Text: {:?}", msg);
        } else {
            console_log!("message event, received Unknown: {:?}", e.data());
        }
    });
    ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();

    // Setup error callback
    let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
        console_log!("error event: {:?}", e);
    });
    ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
    onerror_callback.forget();

    // Setup open callback
    let cloned_ws = ws.clone();
    let onopen_callback = Closure::<dyn FnMut()>::new(move || {
        console_log!("socket opened");

        let msg = serde_json::to_string(&ClientMessage::SetName("Dento".to_owned())).unwrap();
        match cloned_ws.send_with_str(&msg) {
            Ok(_) => console_log!("message successfully sent"),
            Err(err) => console_log!("error sending message: {:?}", err),
        }

        let msg = serde_json::to_string(&ClientMessage::CreateGame).unwrap();
        match cloned_ws.send_with_str(&msg) {
            Ok(_) => console_log!("message successfully sent"),
            Err(err) => console_log!("error sending message: {:?}", err),
        }
    });
    ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();
}
