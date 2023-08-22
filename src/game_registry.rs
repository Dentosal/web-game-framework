use std::collections::HashMap;

use crate::game_state::Game;

#[derive(Default)]
pub struct GameRegistry {
    pub games: HashMap<String, fn() -> Box<dyn Game>>,
}
