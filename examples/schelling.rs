#![deny(unused_must_use)]

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use warp::Filter;

use wgfw::{
    game_state::{Game, GameCommon},
    Builder, PlayerId,
};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProposePermission {
    #[default]
    All,
    Leader,
    No,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReadyPermission {
    All,
    Leader,
    #[default]
    Majority,
    Single,
    No,
}

/// List of predefined questions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct QuestionList {
    enabled: bool,
    url: String,
    name: String,
}

/// List of predefined questions
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QuestionOrder {
    #[default]
    Random,
    Fifo,
    Lifo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameSettings {
    /// Percentage of players that need to be ready before starting the timer,
    /// integer between 0 and 100.
    percentage: u8,
    /// Timer duration in seconds. Runs after `percentage` of players are ready.
    timer: u16,
    /// Who can propose questions
    propose: ProposePermission,
    /// Which question lists are enabled
    question_lists: Vec<QuestionList>,
    /// Order of questions
    order: QuestionOrder,
    /// Anonymize answers
    anonymize: bool,
    /// Who can start the next round
    ready: ReadyPermission,
    /// Minimum delay between rounds, in seconds
    delay: u16,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            percentage: 51,
            timer: 60,
            propose: ProposePermission::default(),
            question_lists: vec![
                QuestionList {
                    enabled: false,
                    url: "/static/lists/chatgpt_en.txt".to_owned(),
                    name: "ChatGPT-generated questions (English)".to_owned(),
                },
                QuestionList {
                    enabled: false,
                    url: "/static/lists/chatgpt_fi.txt".to_owned(),
                    name: "ChatGPT-generated questions (Finnish)".to_owned(),
                },
            ],
            order: QuestionOrder::default(),
            anonymize: false,
            ready: ReadyPermission::default(),
            delay: 5,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Schelling {
    pub settings: GameSettings,
    pub nicknames: HashMap<PlayerId, String>,
    pub history: Vec<Round>,
    pub current_round: Option<Round>,
    pub question_queue: Vec<Question>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Round {
    pub question: Question,
    pub guesses: HashMap<PlayerId, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Question {
    /// Unlimitted possible answers
    Open(String),
    /// Limited possible answers
    Choice(String, Vec<String>),
}

#[derive(Debug, Serialize, Deserialize)]
struct PublicSchelling {
    pub settings: GameSettings,
    pub nicknames: HashMap<PlayerId, String>,
    pub history: Vec<Round>,
    pub current_round: Option<CurrentRoundPublic>,
    pub question_queue: Vec<Question>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurrentRoundPublic {
    pub question: Question,
    pub guessed: HashSet<PlayerId>,
}

impl Schelling {
    pub fn public(&self) -> PublicSchelling {
        let current_round = self.current_round.as_ref().map(|round| CurrentRoundPublic {
            question: round.question.clone(),
            guessed: round.guesses.keys().copied().collect(),
        });

        PublicSchelling {
            settings: self.settings.clone(),
            nicknames: self.nicknames.clone(),
            history: self.history.clone(),
            current_round,
            question_queue: self.question_queue.clone(),
        }
    }
}

/// Message sent by the client
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UserMessage {
    /// Change nickname
    Nick(String),
    /// Propose a new question
    Question(Question),
    /// Guess the answer to the current question
    Guess(String),
    /// Update settings (game leader only)
    Settings(GameSettings),
}

impl Game for Schelling {
    fn public_state(&self, _common: &GameCommon) -> serde_json::Value {
        serde_json::to_value(self.public()).unwrap()
    }

    fn state_for_player(&self, _common: &GameCommon, player: PlayerId) -> serde_json::Value {
        let private = self
            .current_round
            .as_ref()
            .and_then(|round| round.guesses.get(&player));
        serde_json::to_value(private).unwrap()
    }

    fn on_message_from(
        &mut self,
        common: &GameCommon,
        player: PlayerId,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        if let Ok(msg) = serde_json::from_value(message) {
            match msg {
                UserMessage::Nick(name) => {
                    self.nicknames.insert(player, name);
                }
                UserMessage::Question(question) => {
                    self.question_queue.push(question);
                }
                UserMessage::Guess(guess) => {
                    self.current_round.as_mut().map(|round| {
                        round.guesses.insert(player, guess);
                    });
                }
                UserMessage::Settings(settings) => {
                    if player == common.leader {
                        self.settings = settings;
                    } else {
                        return Err("Only leader can change settings".into());
                    }
                }
            }
            Ok(().into())
        } else {
            Err("Invalid message!!".into())
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let index = warp::get()
        .and(warp::path::end())
        .and(warp::fs::file("./examples/schelling_static/index.html"));

    let favicon = warp::get()
        .and(warp::path("favicon.ico"))
        .and(warp::fs::file(
            "./examples/schelling_static/images/favicon.ico",
        ));

    let static_files = warp::path("static").and(warp::fs::dir("./examples/schelling_static/"));

    let (game_server, ws) = Builder::new()
        .register::<Schelling>("schelling")
        // .register::<AntiSchelling>("anti-schelling")
        .spawn();

    let web_server =
        warp::serve(index.or(favicon).or(static_files).or(ws)).run(([127, 0, 0, 1], 3030));

    let ((), game_server_result) = tokio::join!(web_server, game_server);
    game_server_result.expect("Game server panicked");
}
