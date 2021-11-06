use crate::{cache, CLIENT, SACHET_BASE};
use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

const CACHE_KIND: &str = "Sachet";

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameEvent {
    // must be first to verify ordering
    pub(crate) metadata: GameEventMetadata,

    pub(crate) player_tags: Vec<String>,
    pub(crate) team_tags: Vec<String>,
    pub(crate) created: DateTime<Utc>,
    #[serde(rename = "type")]
    pub(crate) ty: u16,
    pub(crate) description: String,

    pub(crate) away_pitcher: Option<String>,
    pub(crate) home_pitcher: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameEventMetadata {
    // play and sub_play must be first to verify ordering
    pub(crate) play: u16,
    pub(crate) sub_play: u16,

    pub(crate) a_player_id: Option<String>,
    pub(crate) b_player_id: Option<String>,
}

pub(crate) async fn load_game_feed(game_id: &str) -> Result<Vec<GameEvent>> {
    let (raw_value, from_cache) = match cache::load(CACHE_KIND, game_id, None).await? {
        None => (
            CLIENT
                .get(format!("{}/packets?id={}", SACHET_BASE, game_id))
                .send()
                .await?
                .bytes()
                .await?,
            false,
        ),
        Some(cached) => (cached.into(), true),
    };

    let mut value: Vec<GameEvent> = serde_json::from_slice(&raw_value)?;
    value.sort_unstable();
    ensure!(
        value.last().map(|event| event.ty) == Some(216),
        "game not over"
    );

    if !from_cache {
        cache::store(CACHE_KIND, game_id, &raw_value, None).await?;
    }

    Ok(value)
}
