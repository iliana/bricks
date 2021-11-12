use crate::stats::Stats;
use crate::{db::Db, ResponseResult};
use anyhow::Result;
use indexmap::IndexMap;
use rocket::get;
use rocket::response::content::Html;
use uuid::Uuid;

#[get("/player/<id>")]
pub(crate) async fn player(db: Db, id: Uuid) -> ResponseResult<Option<Html<String>>> {
    load_player(db, id).await?;
    // TODO stuff
    Ok(None)
}

#[derive(Debug, Default)]
struct PlayerLoad {
    regular_season: IndexMap<RowKey, RowValue>,
    postseason: IndexMap<RowKey, RowValue>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct RowKey {
    sim: String,
    season: u16,
    team: Uuid,
}

#[derive(Debug, Default)]
struct RowValue {
    batting_games: usize,
    pitching_games: usize,
    stats: Stats,
}

async fn load_player(db: Db, id: Uuid) -> Result<Option<PlayerLoad>> {
    db.run(move |conn| {
        let mut load = PlayerLoad::default();
        let mut not_empty = false;
        let mut statement = conn.prepare_cached(
            "SELECT team_id, sim, season, day, stats_json FROM player_stats WHERE player_id = ?",
        )?;
        let rows = statement.query_map([&id], |row| {
            Ok((
                RowKey {
                    sim: row.get("sim")?,
                    season: row.get::<&str, u16>("season")? + 1,
                    team: row.get("team_id")?,
                },
                row.get::<&str, u16>("day")?,
                row.get::<&str, String>("stats_json")?,
            ))
        })?;
        for row in rows {
            not_empty = true;
            let (row_key, day, stats_json) = row?;
            let stats: Stats = serde_json::from_str(&stats_json)?;
            let map = if (day + 1) > 99 {
                &mut load.postseason
            } else {
                &mut load.regular_season
            };
            let entry = map.entry(row_key).or_default();
            if stats.is_batting() {
                entry.batting_games += 1;
            }
            if stats.is_pitching() {
                entry.pitching_games += 1;
            }
            entry.stats += stats;
        }
        Ok(not_empty.then(|| load))
    })
    .await
}
