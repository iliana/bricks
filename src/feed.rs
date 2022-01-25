use crate::{CLIENT, DB, SACHET_BASE};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

pub async fn load(game_id: Uuid) -> Result<Vec<GameEvent>> {
    let tree = DB.open_tree("cache_sachet_v1")?;
    if let Some(data) = tree.get(game_id.as_bytes())? {
        let mut events: Vec<GameEvent> = serde_json::from_slice(&data)?;
        events.sort_unstable();
        if check(&events) {
            return Ok(events);
        } else {
            log::warn!("removing cached feed for {}", game_id);
            tree.remove(game_id.as_bytes())?;
        }
    }

    let data = CLIENT
        .get(format!("{}/packets?id={}", SACHET_BASE, game_id))
        .send()
        .await?
        .text()
        .await?;
    let mut events: Vec<GameEvent> = serde_json::from_str(&data)?;
    events.sort_unstable();
    // if this check fails, return anyway so we can get debug output, but don't cache
    if check(&events) {
        tree.insert(game_id.as_bytes(), data.into_bytes())?;
    } else {
        log::warn!("not caching feed for {}", game_id);
    }
    Ok(events)
}

fn check(feed: &[GameEvent]) -> bool {
    if feed.is_empty() {
        return false;
    }
    let mut expected = (0, 0);
    for event in feed {
        expected = match event.expect(expected) {
            Ok(x) => x,
            Err(_) => return false,
        };
    }
    feed.iter()
        .rev()
        .any(|event| event.ty == 214 || event.ty == 215)
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct GameEvent {
    // must be first to sort
    pub metadata: GameEventMetadata,

    pub id: Uuid,
    pub player_tags: Vec<Uuid>,
    pub team_tags: Vec<Uuid>,
    pub created: DateTime<Utc>,
    pub day: u16,
    pub season: u16,
    #[serde(rename = "type")]
    pub ty: u16,
    pub description: String,

    pub base_runners: Option<Vec<Uuid>>,
    pub bases_occupied: Option<Vec<u16>>,

    pub away_pitcher: Option<Uuid>,
    pub away_pitcher_name: Option<String>,
    pub home_pitcher: Option<Uuid>,
    pub home_pitcher_name: Option<String>,
}

impl GameEvent {
    pub fn expect(&self, expected: (u16, u16)) -> Result<(u16, u16)> {
        if expected != (self.metadata.play, self.metadata.sub_play) {
            // handle empty event before half-inning changes
            if self.ty == 2 && expected == (self.metadata.play - 1, self.metadata.sub_play) {
                Ok(self.next())
            } else {
                bail!("missing event {:?}", expected);
            }
        } else {
            Ok(self.next())
        }
    }

    fn next(&self) -> (u16, u16) {
        if usize::from(self.metadata.sub_play) + 1 == self.metadata.sibling_ids.len() {
            (self.metadata.play + 1, 0)
        } else {
            (self.metadata.play, self.metadata.sub_play + 1)
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct GameEventMetadata {
    // play and sub_play must be first to sort
    pub play: u16,
    pub sub_play: u16,
    pub sibling_ids: Vec<Uuid>,

    pub r#mod: Option<String>,
    pub weather: Option<u16>,
    pub winner: Option<Uuid>,

    #[serde(flatten)]
    pub extra: Option<ExtraData>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub enum ExtraData {
    Swap(PlayerSwapData),
    Incineration(IncinerationReplacementData),
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSwapData {
    pub a_player_id: Uuid,
    pub a_player_name: String,
    pub b_player_id: Uuid,
    pub b_player_name: String,
    pub team_id: Uuid,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct IncinerationReplacementData {
    pub in_player_id: Uuid,
    pub in_player_name: String,
    pub out_player_id: Uuid,
    pub team_id: Uuid,
}
