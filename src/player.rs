use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Player, independent of browser session
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct PlayerId(pub(crate) Uuid);
impl PlayerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// A secret reconnection token, used to identify a player when reconnecting
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReconnectionSecret(orion::auth::Tag);

impl ReconnectionSecret {
    pub fn for_player(key: &orion::auth::SecretKey, player_id: PlayerId) -> Self {
        Self(
            orion::auth::authenticate(key, player_id.0.as_bytes())
                .expect("Unable to sign reconnection secret"),
        )
    }
}
