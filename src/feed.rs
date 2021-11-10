use crate::{db::Db, CLIENT, SACHET_BASE};
use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

const CACHE_KIND: &str = "Sachet";

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameEvent {
    // must be first to verify ordering
    pub(crate) metadata: GameEventMetadata,

    pub(crate) id: Uuid,
    pub(crate) player_tags: Vec<Uuid>,
    pub(crate) team_tags: Vec<Uuid>,
    pub(crate) created: DateTime<Utc>,
    #[serde(rename = "type")]
    pub(crate) ty: u16,
    pub(crate) description: String,

    #[serde(flatten)]
    pub(crate) pitcher_data: Option<PitcherData>,
    pub(crate) base_runners: Option<Vec<Uuid>>,
    pub(crate) bases_occupied: Option<Vec<u16>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameEventMetadata {
    // play and sub_play must be first to verify ordering
    pub(crate) play: u16,
    pub(crate) sub_play: u16,
    pub(crate) sibling_ids: Vec<Uuid>,

    pub(crate) a_player_id: Option<Uuid>,
    pub(crate) b_player_id: Option<Uuid>,
    pub(crate) winner: Option<Uuid>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PitcherData {
    pub(crate) away_pitcher: Uuid,
    pub(crate) away_pitcher_name: String,
    pub(crate) home_pitcher: Uuid,
    pub(crate) home_pitcher_name: String,
}

impl GameEvent {
    pub(crate) fn risp(&self) -> bool {
        self.bases_occupied.iter().flatten().any(|base| *base >= 1)
    }
}

pub(crate) async fn load_game_feed(db: &Db, game_id: Uuid) -> Result<Vec<GameEvent>> {
    let (raw_value, from_cache) = match db.cache_load(CACHE_KIND, game_id.to_string(), None).await?
    {
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
        db.cache_store(CACHE_KIND, game_id.to_string(), &raw_value, None)
            .await?;
    }

    Ok(value)
}
