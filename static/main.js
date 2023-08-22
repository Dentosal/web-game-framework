const main = WgfwEvents => {
    document.getElementById("global-banner").innerHTML = "Connecting to server...";

    let events = new WgfwEvents();

    events.onready = async () => {
        console.log("onready");
        let game_modes = await events.game_modes();
        console.log("game_modes", game_modes);
        let game_id = await events.create_game("chat");
        console.log("game_id", game_id);

        let inner = await events.inner(game_id, "Test message");
        console.log("inner", inner);
    };

    events.onupdate = (gameId, leader, players, public, _private) => {
        console.log("onupdate", gameId, leader, players, public, _private);
    };

    // events.on("connect", () => {
    //     console.log("Connected to server");
    //     document.getElementById("global-banner").innerHTML = "Connected to server";
    // });
}
