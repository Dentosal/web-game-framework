use std::collections::HashMap;

use crate::game_state::Game;

type Constructor = Box<dyn Fn() -> Box<dyn Game> + Send + Sync>;

#[derive(Default)]
pub struct GameRegistry {
    pub games: HashMap<String, Constructor>,
}

impl GameRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, game: Constructor) {
        self.games.insert(name.to_owned(), game);
    }
}
