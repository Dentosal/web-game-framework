#![deny(unused_must_use)]
#![feature(hash_drain_filter)]

use std::net::SocketAddr;

use warp::Filter;

mod game_server;

use self::game_server::{ClientHandle, ServerRemote};

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

    let (jh, game_server_handle) = game_server::spawn();

    let ws = warp::path("ws")
        .and(warp::ws())
        .and(with_game_server(game_server_handle))
        .map(|ws: warp::ws::Ws, ch: ClientHandle| ws.on_upgrade(|s| ch.handle_ws_client(s)));

    warp::serve(index.or(favicon).or(static_files).or(ws))
        .run(([127, 0, 0, 1], 3030))
        .await;

    jh.await.expect("Game server paniced");
}

fn with_game_server(
    handle: ServerRemote,
) -> impl Filter<Extract = (ClientHandle,), Error = std::convert::Infallible> + Clone {
    warp::any()
        .and(warp::addr::remote())
        .map(move |remote_addr: Option<SocketAddr>| handle.make_client_handle(remote_addr.unwrap()))
}
