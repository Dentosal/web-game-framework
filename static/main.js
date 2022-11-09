let ws;

const show_error = html => {
    let banner = document.getElementById("global-banner");
    banner.innerHTML = html;
    banner.classList.add("error");
};

const send_message = data => {
    ws.send(JSON.stringify(data));
};

const main = () => {
    document.getElementById("global-banner").innerHTML = "Connecting to server...";

    ws = new WebSocket("ws://localhost:3030/ws");

    ws.onopen = function (e) {
        document.getElementById("global-banner").innerHTML = "";
        let player_name = localStorage.getItem("player_name");
        if (player_name == null) {
            show_start_screen();
        } else {
            set_player_name(player_name);
        }
    };

    ws.onmessage = function (event) {
        let data = JSON.parse(event.data);
        console.log("incoming", data);
        if ("game_info" in data) {
            window.location.hash = data.game_info.id;
            show_game(data.game_info);
        }
    };

    ws.onclose = function (event) {
        console.log(event);
        if (event.wasClean) {
            show_error(`[close] Connection closed cleanly, code=${event.code} reason=${event.reason}`);
        } else {
            show_error("Connection closed (error)");
        }
    };

    ws.onerror = function (error) {
        console.log(error);
        show_error("Connection error");
    };
};

const show_start_screen = () => {
    let ss = document.getElementById("start-screen");
    ss.classList.remove("nodisplay");
    ss.querySelector("input[type='text']").focus();
};

const submit_player_name = () => {
    let nick = document.querySelector("#start-screen input[type='text']").value;
    set_player_name(nick);
    return false; // No reload on form submit
};

const set_player_name = nick => {
    if (nick !== "") {
        document.getElementById("start-screen").classList.add("nodisplay");

        localStorage.setItem("player_name", nick);
        send_message({"set_name": nick});
        let game_share_code = window.location.hash.slice(1);
        const re_uuid = /^[0-9a-fA-F]{8}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{12}$/gi;
        if (game_share_code.match(re_uuid)) {
            send_message({ "join_game": game_share_code });
        } else {
            send_message({ "join_game": null });
        }
    }
};

const update_game_link = () => {
    document.getElementById("game-link").value = window.location.href;
    let qr = document.getElementById("link-qrcode");
    qr.innerHTML = "";
    new QRCode(qr, window.location.href, {
        width: 64,
        height: 64,
    });
}

const start_game = () => {
    send_message({ "start_game": null });
};

const show_game = (game_data) => {
    let ss = document.getElementById("game-screen");
    ss.classList.remove("nodisplay");
    let is_game_leader = game_data.info.you === game_data.info.leader;
    ss.setAttribute("data-leader", is_game_leader);
    let is_in_game = game_data.info.state !== null;
    ss.setAttribute("data-in-game", is_in_game);

    update_game_link();

    if (game_data.info.state === null) {
        document.getElementById("game-state").innerHTML = "In lobby";
    }

    let pl = document.getElementById("players");
    let children = [];

    let elem = document.createElement("div");
    let f0 = document.createElement("div");
    f0.setAttribute("data-if-role", "leader");
    f0.innerText = "Actions";
    elem.appendChild(f0);
    let f1 = document.createElement("div");
    f1.innerText = "Player";
    elem.appendChild(f1);
    children.push(elem);

    for (player_id of game_data.info.players) {
        let player_name = game_data.info.player_names[player_id];

        let elem = document.createElement("div");
        elem.setAttribute("data-player-id", player_id);
        elem.setAttribute("data-player-name", player_name);
        elem.setAttribute("data-current-player", game_data.info.you === player_id);
        elem.setAttribute("data-game-leader", game_data.info.leader === player_id);

        let f0 = document.createElement("div");
        f0.setAttribute("data-if-role", "leader");
        f0.innerHTML = "<button>Kick</button>";
        elem.appendChild(f0);

        let f1 = document.createElement("div");
        f1.innerText = player_name;
        elem.appendChild(f1);
        children.push(elem);
    }

    pl.replaceChildren(...children);
};
