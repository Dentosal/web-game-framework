const Page = {
    events: null,
    connected: false,
    me: null,
    chats: {},
    metas: {},
    activeChat: null,
    myNick: "",
    newChatTitle: "",

    start() {
        this.events = new window.WgfwEvents();
        this.events.onready = async (playerId) => {
            this.connected = true;
            this.me = playerId;
            // Fetch all games
            let joined = await this.events.joined_games();

            if (joined.length === 0) {
                let newChat = await this.events.create_game("chat");
                await this.events.inner(newChat, { "title": "Welcome" });
            }
        };

        this.events.onupdate = (gameId, leader, players, pub, _priv) => {
            console.log("onupdate", gameId, leader, players, pub, _priv);
            this.chats[gameId] = pub;
            this.metas[gameId] = {
                leader,
                players,
            };
            if (Object.keys(this.chats).length === 1) {
                this.activeChat = gameId;
            }
        };

    },

    async newChat() {
        let newChat = await this.events.create_game("chat");
        await this.events.inner(newChat, { "title": this.newChatTitle });
        this.activeChat = newChat;
        this.newChatTitle = "";
    },

    async sendMessage(elem) {
        await this.events.inner(this.activeChat, {"chat": elem.value});
        elem.value = "";
    },

    async updateNick() {
        // Nick is global, so we need to update all chats
        for (chat in this.chats) {
            await this.events.inner(chat, {"nick": this.myNick});
        }
    },

    async updateChatTitle() {
        await this.events.inner(this.activeChat, { "title": this.chats[this.activeChat].title });
    },
};