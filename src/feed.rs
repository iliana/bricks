use crate::{API_BASE, CLIENT, DB};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, ToSql};
use serde::Deserialize;

const CACHE_KIND: &str = "GameFeed";

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameEvent {
    // must be first to verify ordering
    metadata: GameEventMetadata,

    player_tags: Vec<String>,
    team_tags: Vec<String>,
    created: DateTime<Utc>,
    #[serde(rename = "type")]
    ty: u16,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
struct GameEventMetadata {
    // play and sub_play must be first to verify ordering
    play: u16,
    sub_play: u16,
}

pub(crate) async fn load_game_feed(game_id: &str) -> Result<Vec<GameEvent>> {
    let guard = DB.lock().await;
    let raw_value: Option<Vec<u8>> = guard
        .query_row(
            "SELECT value FROM caches WHERE kind = :kind AND key = :key",
            &[(":kind", &CACHE_KIND as &dyn ToSql), (":key", &game_id)],
            |row| row.get(0),
        )
        .optional()?;
    drop(guard);

    let (raw_value, from_cache) = match raw_value {
        None => (
            CLIENT
                .get(format!(
                    "{}/database/feed/game?id={}&sort=1",
                    API_BASE, game_id
                ))
                .send()
                .await?
                .bytes()
                .await?,
            false,
        ),
        Some(v) => (zstd::decode_all(v.as_slice())?.into(), true),
    };

    let mut value: Vec<GameEvent> = serde_json::from_slice(&raw_value)?;
    value.sort_unstable();

    if !from_cache {
        let compressed_value = zstd::encode_all(&*raw_value, 0)?;
        let guard = DB.lock().await;
        guard.execute(
            "INSERT INTO caches (kind, key, value) VALUES (:kind, :key, :value)",
            &[
                (":kind", &CACHE_KIND as &dyn ToSql),
                (":key", &game_id),
                (":value", &compressed_value),
            ],
        )?;
    }

    Ok(value)
}
