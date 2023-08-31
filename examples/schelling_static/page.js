const copyToClipboard = elem => {
    navigator.clipboard.writeText(elem.value);

    let button = elem.parentElement.querySelector("input[type=button]");
    button.value = "Copied!";
    setTimeout(() => {
        button.value = "Copy";
    }, 1000);
}

var timerTask = null;
var delayTask = null;


const Page = {
    error: null,
    events: null,
    me: null,
    gameModes: [],
    gameId: null,
    myNick: window.localStorage.getItem("nick") || null,
    state: {
        settings: {},
        history: [],
        question_queue: [],
    },
    currentGuess: null,
    leader: null,
    players: [],
    expandSettings: true,
    timerSeconds: null,
    delaySeconds: null,

    start() {
        this.events = new window.WgfwEvents();
        this.events.onready = async (playerId) => {
            this.connected = true;
            this.me = playerId;
            // Fetch all games
            let join_hash = window.location.hash.match(/#join:([0-9a-f-]+)$/);
            if (join_hash) {
                await this.setActiveGame(join_hash[1]);
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

            if (!_.isEqual(this.state.settings, pub.settings)) {
                // Show settings if they have changed
                this.expandSettings = true;
            } else if (!this.state.running && pub.running) {
                // Hide settings if the game has just started
                this.expandSettings = false;
            }

            this.state = pub;
            this.currentGuess = priv;
            this.leader = leader;
            this.players = players;

            this.updateSettingsWritable();
            this.updateTimers();
        };

        this.events.onerror = err => {
            this.error = "Connection to the server closed" + (err ? ": " + err : "");
        };

        window.onhashchange = async () => {
            let join_hash = window.location.hash.match(/#join:([0-9a-f-]+)$/);
            if (join_hash) {
                this.setActiveGame(join_hash[1]);
                window.location.hash = "";
            }
        };
    },

    updateTimers() {
        if (this.state.timer_from !== null) {
            timerTask = setInterval(() => {
                let end = new Date(this.state.timer_from + this.state.settings.timer * 1000);
                let value = Math.round((end - new Date()) / 1000);
                this.timerSeconds = Math.max(value, 0);
                if (value > 0) {
                    this.timerSeconds = value;
                } else {
                    this.timerSeconds = null;
                    clearInterval(timerTask);
                }
            }, 1000);
        } else {
            if (timerTask !== null) {
                clearInterval(timerTask);
            }
            timerTask = null;
        }

        if (this.state.delay_from !== null) {
            delayTask = setInterval(() => {
                let end = new Date(this.state.delay_from + this.state.settings.delay * 1000);
                let value = Math.round((end - new Date()) / 1000);
                if (value > 0) {
                    this.delaySeconds = value;
                } else {
                    this.delaySeconds = null;
                    clearInterval(delayTask);
                }
            }, 1000);
        } else {
            if (delayTask !== null) {
                clearInterval(delayTask);
            }
            delayTask = null;
        }
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

        // Update the game QR code
        new QRCode(document.getElementById("link-qrcode"), this.gameLink(), {
                width: 64,
                height: 64,
            });

        this.updateNick();
    },

    gameLink() {
        return window.location.origin + '/#join:' + this.gameId
    },

    async createGame(mode) {
        this.gameId = await this.events.create_game(mode);
        this.updateNick();
    },

    async trySetNick(nick) {
        if (nick.length === 0) {
            return;
        }

        this.myNick = nick;
        window.localStorage.setItem("nick", this.myNick);
        this.updateNick();
    },

    async updateNick() {
        if (this.gameId !== null) {
            await this.events.inner(this.gameId, {"nick": this.myNick});
        }
    },

    async updateSettings() {
        if (this.gameId !== null && this.me == this.leader) {
            console.log("Updating settings");
            await this.events.inner(this.gameId, {"settings": this.state.settings});
            console.log("Updated");
        }
    },

    // Makes settings writable if the user is the leader, and disables them otherwise.
    async updateSettingsWritable() {
        if (this.gameId !== null) {
            document.querySelectorAll("#gamesettings *:not(legend) input").forEach(elem => {
                elem.disabled = (this.me !== this.leader);
            });
        }
    },

    async startGame() {
        await this.events.inner(this.gameId, "start");
    },

    async sendProposal(elem) {
        await this.events.inner(this.gameId, { question: {open: elem.value} });
        elem.value = "";
    },

    async sendAnswer(elem) {
        await this.events.inner(this.gameId, { guess: elem.value });
        elem.value = "";
    },

    needsReady() {
        if (this.state.ready.includes(this.me)) {
            return false;
        }

        switch (this.state.settings.ready) {
            case "all": return true;
            case "leader": return this.me == this.leader;
            case "majority": return true;
            case "single": return true;
            case "no": return false;
        }
    },

    async sendReady() {
        await this.events.inner(this.gameId, "ready");
    },
};