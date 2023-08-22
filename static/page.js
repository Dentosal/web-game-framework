const Page = {
    events: null,
    connected: false,
    chats: {},
    activeChat: null,
    newChatTitle: "",

    start() {
        this.events = new window.WgfwEvents();
        this.events.onready = async () => {
            this.connected = true;
            let game_modes = await this.events.game_modes();
            console.log("game_modes", game_modes);
            let game_id = await this.events.create_game("chat");
            console.log("game_id", game_id);
        };

        this.events.onupdate = (gameId, leader, players, pub, _priv) => {
            console.log("onupdate", gameId, leader, players, pub, _priv);
            this.chats[gameId] = pub;
            if (Object.keys(this.chats).length === 1) {
                this.activeChat = gameId;
            }
        };

    },

    async newChat() {
        let newChat = await this.events.create_game("chat");
        await this.events.inner(this.activeChat, { "title": this.newChatTitle });
        this.activeChat = newChat;
        this.newChatTitle = "";
    },

    async sendMessage(elem) {
        await this.events.inner(this.activeChat, {"chat": elem.value});
        elem.value = "";
    },
};