const main = WgfwEvents => {
    document.getElementById("global-banner").innerHTML = "Connecting to server...";

    let events = new WgfwEvents();

    events.onready = async () => {
        console.log("onready");
        let game_id = await events.create_game("chat");
        console.log("game_id", game_id);
    };

    events.onupdate = (data) => {
        console.log("onupdate", data);
    };

    // events.on("connect", () => {
    //     console.log("Connected to server");
    //     document.getElementById("global-banner").innerHTML = "Connected to server";
    // });
}
