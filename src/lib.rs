#![deny(unused_must_use)]

use std::net::SocketAddr;

use game_state::Game;
use tokio::task::JoinHandle;
use warp::{Filter, Rejection, Reply};

mod event_queue;
mod game_registry;
mod game_server;
pub mod game_state;

pub use self::game_registry::GameRegistry;
pub use wgfw_protocol as protocol;
pub use wgfw_protocol::{GameId, PlayerId, ReconnectionSecret};

use self::game_server::{ClientHandle, ServerRemote};

#[derive(Default)]
pub struct Builder {
    registry: GameRegistry,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<G: Game + Default + 'static>(mut self, name: &str) -> Self {
        self.registry
            .register(name, Box::new(|| Box::<G>::default()));
        self
    }

    pub fn register_by_contructor(
        mut self,
        name: &str,
        game: fn() -> Box<dyn game_state::Game>,
    ) -> Self {
        self.registry.register(name, Box::new(game));
        self
    }

    pub fn spawn(
        self,
    ) -> (
        JoinHandle<()>,
        impl warp::Filter<Extract = impl Reply, Error = Rejection> + Clone,
    ) {
        let Self { registry } = self;
        let (jh, game_server_handle) = game_server::spawn(registry);

        let wasm_bg = warp::path("wasm")
            .and(warp::path("wgfw_wasm_bg.wasm"))
            .map(|| warp::reply::with_header(WASM_BG, "content-type", "application/wasm"));

        let wasm_js = warp::path("wasm")
            .and(warp::path("wgfw_wasm.js"))
            .map(|| warp::reply::with_header(WASM_JS, "content-type", "text/javascript"));

        let wasm = wasm_bg.or(wasm_js);

        let ws = warp::path("ws")
            .and(warp::ws())
            .and(with_game_server(game_server_handle))
            .map(|ws: warp::ws::Ws, ch: ClientHandle| ws.on_upgrade(|s| ch.handle_ws_client(s)));

        (jh, wasm.or(ws))
    }
}

fn with_game_server(
    handle: ServerRemote,
) -> impl Filter<Extract = (ClientHandle,), Error = std::convert::Infallible> + Clone {
    warp::any()
        .and(warp::addr::remote())
        .map(move |remote_addr: Option<SocketAddr>| handle.make_client_handle(remote_addr.unwrap()))
}

// WASM pkg files compiled by build.rs
static WASM_BG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/wasm/pkg/wgfw_wasm_bg.wasm"
));
static WASM_JS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/wasm/pkg/wgfw_wasm.js"
));
