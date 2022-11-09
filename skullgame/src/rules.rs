use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tile {
    Skull,
    Flower,
}

type TileCount = usize;

/// Full game information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GamePrivate {
    players: Vec<PlayerPrivate>,
    actions: Vec<ActionPrivate>,
}
impl GamePrivate {
    pub fn new(player_count: usize) -> Self {
        Self {
            players: vec![
                PlayerPrivate {
                    has_point: false,
                    tiles: 4,
                    has_skull: true,
                    skull_known: true,
                };
                player_count
            ],
            actions: Vec::new(),
        }
    }

    pub fn for_player(&self, player: usize) -> (GamePublic, PlayerPrivate) {
        (
            GamePublic {
                players: self.players.iter().map(|p| p.public()).collect(),
                actions: self.actions.iter().map(|p| p.public()).collect(),
            },
            self.players[player],
        )
    }
}

/// Fully public game information
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GamePublic {
    players: Vec<PlayerPublic>,
    actions: Vec<ActionPublic>,
}
impl GamePublic {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerPrivate {
    has_point: bool,
    tiles: TileCount,
    has_skull: bool,
    skull_known: bool,
}
impl PlayerPrivate {
    pub fn public(self) -> PlayerPublic {
        PlayerPublic {
            has_point: self.has_point,
            tiles: self.tiles,
            has_skull: if self.skull_known {
                Some(self.has_skull)
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerPublic {
    has_point: bool,
    tiles: TileCount,
    has_skull: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionPublic {
    Place,
    Challenge(Option<TileCount>),
    Reveal(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionPrivate {
    Place(Tile),
    Challenge(Option<TileCount>),
    Reveal(usize),
}
impl ActionPrivate {
    pub fn public(self) -> ActionPublic {
        match self {
            Self::Place(_) => ActionPublic::Place,
            Self::Challenge(n) => ActionPublic::Challenge(n),
            Self::Reveal(n) => ActionPublic::Reveal(n),
        }
    }
}
