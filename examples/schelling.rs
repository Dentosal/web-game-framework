#![deny(unused_must_use)]

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use rand::Rng;
use serde::{Deserialize, Serialize};
use warp::Filter;

use wgfw::{
    game_state::{EventId, Game, GameCommon, Updates},
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
    #[serde(skip)]
    cached: Option<Vec<String>>,
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
                    cached: None,
                },
                QuestionList {
                    enabled: false,
                    url: "/static/lists/chatgpt_fi.txt".to_owned(),
                    name: "ChatGPT-generated questions (Finnish)".to_owned(),
                    cached: None,
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
    pub history: Vec<HistoryRound>,
    pub current_round: Option<Round>,
    pub question_queue: Vec<Question>,
    pub running: bool,
    /// After-the-round delay is active
    pub delay: bool,
}

fn normalize_guess(guess: &str) -> String {
    guess.trim().to_lowercase()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Round {
    pub question: Question,
    pub guesses: HashMap<PlayerId, String>,
}

impl Round {
    fn into_history(self, anonymize: bool) -> HistoryRound {
        let answers = self.guesses.values().map(|guess| normalize_guess(guess));

        HistoryRound {
            question: self.question,
            guesses: if anonymize {
                let mut answers = answers.map(|a| (a, 0)).collect::<HashMap<_, _>>();
                for (_, v) in &self.guesses {
                    *answers.get_mut(&normalize_guess(v)).unwrap() += 1;
                }
                RoundGuesses::Anonymized(answers)
            } else {
                let mut answers = answers
                    .map(|a| (a, HashSet::new()))
                    .collect::<HashMap<_, _>>();

                for (id, v) in self.guesses.into_iter() {
                    answers.get_mut(&normalize_guess(&v)).unwrap().insert(id);
                }

                RoundGuesses::Full(answers)
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryRound {
    pub question: Question,
    pub guesses: RoundGuesses,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RoundGuesses {
    Anonymized(HashMap<String, usize>),
    Full(HashMap<String, HashSet<PlayerId>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub history: Vec<HistoryRound>,
    pub current_round: Option<CurrentRoundPublic>,
    pub question_queue: Vec<Question>,
    pub running: bool,
    pub delay: bool,
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
            running: self.running,
            delay: self.delay,
        }
    }

    /// Advance to the next phase or round, if needed
    pub fn update(&mut self, common: &GameCommon) {
        // Check if current round is finished
        if let Some(round) = self.current_round.as_ref() {
            let percentage_answered = (round.guesses.len() as f32) / (common.players.len() as f32);
            if percentage_answered >= (self.settings.percentage as f32) / 100.0 {
                // Round is finished
                self.history
                    .push(round.clone().into_history(self.settings.anonymize));
                self.current_round = None;
                self.delay = true;
            }
        }

        if self.current_round.is_none() && self.running && !self.delay {
            // Start new round
            let mut rng = rand::thread_rng();
            if self.question_queue.is_empty() {
                // Pick a random question from the list if any are enabled
                // TODO
                // let mut enabled_lists = self
                //     .settings
                //     .question_lists
                //     .iter()
                //     .filter(|list| list.enabled);
            } else {
                // Pick from proposal queue if it's not empty
                let i = match self.settings.order {
                    QuestionOrder::Random => rng.gen_range(0..self.question_queue.len()),
                    QuestionOrder::Fifo => 0,
                    QuestionOrder::Lifo => self.question_queue.len() - 1,
                };
                self.current_round = Some(Round {
                    question: self.question_queue.remove(i),
                    guesses: HashMap::new(),
                });
            }
        }
    }
}

/// Message sent by the client
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UserMessage {
    /// Change nickname
    Nick(String),
    /// Update settings (game leader only)
    Settings(GameSettings),
    /// Propose a new question
    Question(Question),
    /// Guess the answer to the current question
    Guess(String),
    /// Start or unpause the game
    Start,
    /// Pause the game
    Pause,
    /// Advance to the next phase or round, if this has not been done automatically
    Advance,
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

    fn on_event(&mut self, common: &GameCommon, _id: EventId) -> Updates {
        self.delay = false;
        self.update(common);
        Updates::CHANGED
    }

    fn on_message_from(
        &mut self,
        common: &GameCommon,
        player: PlayerId,
        message: serde_json::Value,
    ) -> (Updates, Result<serde_json::Value, serde_json::Value>) {
        if let Ok(msg) = serde_json::from_value(message) {
            let had_delay_before = self.delay;

            match msg {
                UserMessage::Nick(name) => {
                    self.nicknames.insert(player, name);
                }
                UserMessage::Settings(settings) => {
                    if player == common.leader {
                        self.settings = settings;
                    } else {
                        return (Updates::NONE, Err("Only leader can change settings".into()));
                    }
                }
                UserMessage::Question(question) => {
                    match self.settings.propose {
                        ProposePermission::All => {}
                        ProposePermission::Leader => {
                            if player != common.leader {
                                return (
                                    Updates::NONE,
                                    Err("Only leader can propose questions".into()),
                                );
                            }
                        }
                        ProposePermission::No => {
                            return (
                                Updates::NONE,
                                Err("Proposing questions is not allowed".into()),
                            );
                        }
                    }
                    self.question_queue.push(question);
                    self.update(common);
                }
                UserMessage::Guess(guess) => {
                    self.current_round.as_mut().map(|round| {
                        round.guesses.insert(player, guess);
                    });
                    self.update(common);
                }
                UserMessage::Start => {
                    if player == common.leader {
                        self.running = true;
                    } else {
                        return (Updates::NONE, Err("Only leader can start the game".into()));
                    }
                    self.update(common);
                }
                UserMessage::Pause => {
                    if player == common.leader {
                        self.running = false;
                    } else {
                        return (Updates::NONE, Err("Only leader can pause the game".into()));
                    }
                }
                UserMessage::Advance => {
                    if player == common.leader {
                        self.update(common);
                    } else {
                        return (
                            Updates::NONE,
                            Err("Only leader can advance the game".into()),
                        );
                    }
                }
            }

            let mut updates = Updates::CHANGED;
            if !had_delay_before && self.delay {
                let _ = updates.add_timeout(Duration::from_secs(self.settings.delay as u64));
            }
            (updates, Ok(().into()))
        } else {
            (Updates::NONE, Err("Invalid message!!".into()))
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
