#![deny(unused_must_use)]

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, SystemTime},
};

use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
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
    name: String,
    path: String,
}

impl QuestionList {
    pub fn read(&self) -> Result<Vec<String>, std::io::Error> {
        let path = std::fs::canonicalize(&self.path)?;

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("examples/schelling_static");

        if !path.starts_with(d) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Path is not inside the static directory",
            ));
        }

        let contents = std::fs::read(&self.path)?;
        Ok(String::from_utf8(contents)
            .expect("Invalid utf-8")
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect())
    }
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
                    name: "ChatGPT-generated questions (English)".to_owned(),
                    path: "./examples/schelling_static/lists/chatgpt_en.txt".to_owned(),
                },
                QuestionList {
                    enabled: false,
                    name: "ChatGPT-generated questions (Finnish)".to_owned(),
                    path: "./examples/schelling_static/lists/chatgpt_fi.txt".to_owned(),
                },
            ],
            order: QuestionOrder::default(),
            anonymize: false,
            ready: ReadyPermission::default(),
            delay: 5,
        }
    }
}

impl GameSettings {
    pub fn timer(&self) -> Duration {
        Duration::from_secs(self.timer as u64)
    }

    pub fn delay(&self) -> Duration {
        Duration::from_secs(self.delay as u64)
    }
}

#[derive(Debug, Default)]
struct Schelling {
    pub running: bool,
    pub settings: GameSettings,
    pub nicknames: HashMap<PlayerId, String>,
    pub history: Vec<HistoryRound>,
    pub current_round: Option<Round>,
    /// Timer after the current round ends.
    pub timer_from: Option<Instant>,
    pub question_queue: Vec<Question>,
    pub ready: HashSet<PlayerId>,
    /// After-the-round delay is active. This is the time last round ended.
    pub delay_from: Option<Instant>,
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

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Question {
    /// Unlimitted possible answers
    Open(String),
    /// Limited possible answers
    Choice(String, Vec<String>),
}

#[derive(Debug, Serialize, Deserialize)]
struct PublicSchelling {
    pub running: bool,
    pub settings: GameSettings,
    pub nicknames: HashMap<PlayerId, String>,
    pub history: Vec<HistoryRound>,
    pub current_round: Option<CurrentRoundPublic>,
    pub question_queue: Vec<Question>,
    pub ready: HashSet<PlayerId>,
    #[serde(with = "serde_millis")]
    pub timer_from: Option<SystemTime>,
    #[serde(with = "serde_millis")]
    pub delay_from: Option<SystemTime>,
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
            ready: self.ready.clone(),
            timer_from: self.timer_from.map(|at| (SystemTime::now() - at.elapsed())),
            delay_from: self.delay_from.map(|at| (SystemTime::now() - at.elapsed())),
        }
    }

    /// Advance to the next phase or round, if needed
    pub fn update(&mut self, common: &GameCommon) -> Updates {
        let mut updates = Updates::NONE;
        let now = Instant::now();

        // Check if current round is finished
        if let Some(round) = self.current_round.as_ref() {
            if round.guesses.len() < common.players.len()
                || self
                    .timer_from
                    .map(|at| at.elapsed() < self.settings.timer())
                    .unwrap_or(false)
            {
                let percentage_answered =
                    (round.guesses.len() as f32) / (common.players.len() as f32);
                if percentage_answered >= (self.settings.percentage as f32) / 100.0 {
                    if self.timer_from.is_none() {
                        updates.state_changed = true;
                        self.timer_from = Some(now);
                        let _ = updates.add_timeout(now + self.settings.timer());
                    }
                }
            } else {
                // Round is finished
                updates.state_changed = true;
                self.history
                    .push(round.clone().into_history(self.settings.anonymize));
                self.current_round = None;
                self.timer_from = None;
                self.delay_from = Some(now);
                let _ = updates.add_timeout(now + self.settings.delay());
            }
        }

        if self.current_round.is_none() && self.running {
            if let Some(delay) = self.delay_from {
                let players_ready = match self.settings.ready {
                    ReadyPermission::All => self.ready.len() == common.players.len(),
                    ReadyPermission::Leader => self.ready.contains(&common.leader),
                    ReadyPermission::Majority => self.ready.len() > common.players.len() / 2,
                    ReadyPermission::Single => !self.ready.is_empty(),
                    ReadyPermission::No => true,
                };

                if players_ready && delay.elapsed() >= self.settings.delay() {
                    updates.state_changed = true;
                    self.delay_from = None;
                    self.ready.clear();
                } else {
                    return updates;
                }
            }

            updates.state_changed = true;

            // Start new round if needed
            let mut rng = rand::thread_rng();
            if self.question_queue.is_empty() {
                // Pick a random question from a list if any enabled
                let mut questions: Vec<_> = self
                    .settings
                    .question_lists
                    .iter_mut()
                    .filter(|list| list.enabled)
                    .map(|list| list.read().expect("Failed to read")) // TODO: handle errors instead
                    .flatten()
                    .collect();

                while !questions.is_empty() {
                    let i = rng.gen_range(0..questions.len());
                    let question = questions.swap_remove(i);
                    let question = Question::Open(question.to_owned());
                    // Reroll if the question was already asked
                    if self.history.iter().any(|round| round.question == question) {
                        continue;
                    }

                    self.current_round = Some(Round {
                        question,
                        guesses: HashMap::new(),
                    });
                    break;
                }
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

        updates
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
    /// Ready to start the next round
    Ready,
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
        self.update(common)
    }

    fn on_message_from(
        &mut self,
        common: &GameCommon,
        player: PlayerId,
        message: serde_json::Value,
    ) -> (Updates, Result<serde_json::Value, serde_json::Value>) {
        if let Ok(msg) = serde_json::from_value(message) {
            let updates = match msg {
                UserMessage::Nick(name) => {
                    self.nicknames.insert(player, name);
                    Updates::CHANGED
                }
                UserMessage::Settings(settings) => {
                    if player == common.leader {
                        self.settings = settings;
                        Updates::CHANGED
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
                    Updates::CHANGED
                }
                UserMessage::Guess(guess) => {
                    self.current_round.as_mut().map(|round| {
                        round.guesses.insert(player, guess);
                    });
                    Updates::CHANGED
                }
                UserMessage::Ready => {
                    if self.ready.insert(player) {
                        Updates::CHANGED
                    } else {
                        Updates::NONE
                    }
                }
                UserMessage::Start => {
                    if player == common.leader {
                        self.running = true;
                        Updates::CHANGED
                    } else {
                        return (Updates::NONE, Err("Only leader can start the game".into()));
                    }
                }
                UserMessage::Pause => {
                    if player == common.leader {
                        self.running = false;
                        Updates::CHANGED
                    } else {
                        return (Updates::NONE, Err("Only leader can pause the game".into()));
                    }
                }
                UserMessage::Advance => {
                    if player == common.leader {
                        Updates::CHANGED
                    } else {
                        return (
                            Updates::NONE,
                            Err("Only leader can advance the game".into()),
                        );
                    }
                }
            };

            (updates.merge(self.update(common)), Ok(().into()))
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
