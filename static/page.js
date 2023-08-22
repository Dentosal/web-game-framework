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
    connected: false,
    me: null,
    chats: {},
    metas: {},
    activeChat: null,
    myNick: window.localStorage.getItem("nick") || null,
    newChatTitle: "",

    start() {
        this.events = new window.WgfwEvents();
        this.events.onready = async (playerId) => {
            this.connected = true;
            this.me = playerId;
            // Fetch all games
            let joined = await this.events.joined_games();

            let join_hash = window.location.hash.match(/#join:([0-9a-f-]+)$/);
            if (join_hash) {
                let join_id = join_hash[1];
                if (!joined.includes(join_id)) {
                    let chat = await this.events.join_game(join_id);
                    await this.events.inner(chat, { "nick": this.myNick });
                    joined.push(chat);
                } else {
                    this.activeChat = join_id;
                }
                window.location.hash = "";
            }

            if (joined.length === 0) {
                let newChat = await this.events.create_game("chat");
                await this.events.inner(newChat, { "nick": this.myNick });
                await this.events.inner(newChat, { "title": "Welcome" });
                this.activeChat = newChat;
            }
        };

        this.events.onupdate = (gameId, leader, players, pub, _priv) => {
            this.chats[gameId] = pub;
            this.metas[gameId] = {
                leader,
                players,
            };
            if (Object.keys(this.chats).length === 1) {
                this.activeChat = gameId;
            }
        };

        window.onhashchange = async () => {
            let join_hash = window.location.hash.match(/#join:([0-9a-f-]+)$/);
            if (join_hash) {
                let join_id = join_hash[1];
                if (!(join_id in this.chats)) {
                    this.activeChat = await this.events.join_game(join_id);
                    await this.events.inner(this.activeChat, { "nick": this.myNick });
                } else {
                    this.activeChat = join_id;
                }
                window.location.hash = "";
            }
        };
    },

    async newChat() {
        let newChat = await this.events.create_game("chat");
        await this.events.inner(newChat, { "title": this.newChatTitle });
        await this.events.inner(newChat, { "nick": this.myNick });
        this.activeChat = newChat;
        this.newChatTitle = "";
    },

    async sendMessage(elem) {
        await this.events.inner(this.activeChat, {"chat": elem.value});
        elem.value = "";
    },

    async updateNick() {
        window.localStorage.setItem("nick", this.myNick);

        // Nick is global, so we need to update all chats
        for (chat in this.chats) {
            await this.events.inner(chat, {"nick": this.myNick});
        }
    },

    async updateChatTitle() {
        await this.events.inner(this.activeChat, { "title": this.chats[this.activeChat].title });
    },
};