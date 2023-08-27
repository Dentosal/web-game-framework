const copyToClipboard = elem => {
    navigator.clipboard.writeText(elem.value);
    let tooltip = document.createElement("div");
    tooltip.classList.add("copied-indicator");
    tooltip.innerText = "Copied!";
    tooltip.style.position = "absolute";
    let rect = elem.getBoundingClientRect();
    tooltip.style.top = (rect.top - rect.height) + "px";
    tooltip.style.left = (rect.left + rect.width/2) + "px";
    document.body.appendChild(tooltip);
    setTimeout(() => {
        tooltip.remove();
    }, 1000);
}

const Page = {
    events: null,
    me: null,
    gameModes: [],
    gameId: null,
    myNick: window.localStorage.getItem("nick") || null,
    state: {
        settings: {}
    },
    currentGuess: null,
    leader: null,
    players: [],

    start() {
        this.events = new window.WgfwEvents();
        this.events.onready = async (playerId) => {
            this.connected = true;
            this.me = playerId;
            // Fetch all games
            let join_hash = window.location.hash.match(/#join:([0-9a-f-]+)$/);
            if (join_hash) {
                this.setActiveGame(join_hash[1]);
            } else {
                await this.setActiveGame(null);
            }
            // Fetch possible game modes
            this.gameModes = await this.events.game_modes();
        };

        this.events.onupdate = (gameId, leader, players, pub, priv) => {
            if (gameId !== this.gameId) {
                console.log("Received update for unknown game: " + gameId);
                return;
            }

            this.state = pub;
            this.currentGuess = priv;
            this.leader = leader;
            this.players = players;

            this.updateSettingsWritable();
        };

        window.onhashchange = async () => {
            let join_hash = window.location.hash.match(/#join:([0-9a-f-]+)$/);
            if (join_hash) {
                this.setActiveGame(join_hash[1]);
                window.location.hash = "";
            }
        };
    },

    // Sets game joinId as the active game, leaving all other games.
    // If joinId is null, rejoins a random game if any exists.
    async setActiveGame(joinId) {
        // Fetch all games
        let joined = await this.events.joined_games();

        // Join any game if joinId is null
        if (!joinId) {
            if (joined.length > 0) {
                joinId = joined[0];
            } else {
                return; // No games to join
            }
        }

        // Leave all games except joinId
        for (let game of joined) {
            if (game != joinId) {
                await this.events.leave_game(game);
            }
        }

        if (joined.includes(joinId)) {
            // Already joined the target game
            this.gameId = joinId;
        } else {
            // Join the given game
            this.gameId = await this.events.join_game(joinId);
        }

        this.updateNick();
    },

    async createGame(mode) {
        this.gameId = await this.events.create_game(mode);
    },

    async trySetNick(nick) {
        if (nick.length === 0) {
            return;
        }

        this.myNick = nick;
        window.localStorage.setItem("nick", this.myNick);
        this.updateNick();
    },

    async updateNick(nick) {
        if (this.gameId !== null) {
            await this.events.inner(this.gameId, {"nick": this.myNick});
        }
    },

    async updateSettings() {
        if (this.gameId !== null && this.me == this.leader) {
            await this.events.inner(this.gameId, {"settings": this.state.settings});
        }
    },

    // Makes settings writable if the user is the leader, and disables them otherwise.
    async updateSettingsWritable() {
        if (this.gameId !== null) {
            document.querySelectorAll("#gamesettings input").forEach(elem => {
                elem.disabled = (this.me !== this.leader);
            });
        }
    },
};