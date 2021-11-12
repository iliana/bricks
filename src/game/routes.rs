use crate::game::LogEntry;
use crate::seasons::SIM_NAMES;
use crate::stats::{AwayHome, GameStats};
use crate::{db::Db, filters, ResponseResult};
use anyhow::Result;
use askama::Template;
use rocket::get;
use rocket::response::content::Html;
use rusqlite::{OptionalExtension, ToSql};
use std::borrow::Cow;
use std::collections::HashMap;
use uuid::Uuid;

#[get("/game/<id>")]
pub(crate) async fn game(db: Db, id: Uuid) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_game(db, id).await? {
        GameLoad::Ok {
            stats,
            sim,
            season,
            day,
        } => Some(Html(
            GamePage {
                box_names: stats.box_names(true),
                short_box_names: stats.box_names(false),
                stats,
                era: SIM_NAMES
                    .read()
                    .await
                    .get(&sim)
                    .map(String::as_str)
                    .unwrap_or("Unknown Era"),
                season: season + 1,
                day: day + 1,
            }
            .render()
            .map_err(anyhow::Error::from)?,
        )),
        GameLoad::Failed => Some(Html(
            GameFailedPage { id }
                .render()
                .map_err(anyhow::Error::from)?,
        )),
        GameLoad::NotFound => None,
    })
}

enum GameLoad {
    Ok {
        stats: AwayHome<GameStats>,
        sim: String,
        season: u16,
        day: u16,
    },
    Failed,
    NotFound,
}

async fn load_game(db: Db, id: Uuid) -> Result<GameLoad> {
    // doubly-nested option! the outer is to detect if a row is present, the inner is to detect if
    // there was an error processing this game
    let data: Option<(Option<Vec<u8>>, String, u16, u16)> = db
        .run(move |conn| {
            conn.query_row(
                "SELECT stats_json_zst, sim, season, day FROM game_stats WHERE game_id = ?",
                &[&id as &dyn ToSql],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
        })
        .await?;
    Ok(match data {
        Some((Some(data), sim, season, day)) => GameLoad::Ok {
            stats: serde_json::from_slice(&zstd::decode_all(&*data)?)?,
            sim,
            season,
            day,
        },
        Some((None, _, _, _)) => GameLoad::Failed,
        None => GameLoad::NotFound,
    })
}

#[derive(Template)]
#[template(path = "game.html")]
struct GamePage<'a> {
    stats: AwayHome<GameStats>,
    box_names: HashMap<Uuid, String>,
    short_box_names: HashMap<Uuid, String>,
    era: &'a str,
    season: u16,
    day: u16,
}

#[derive(Template)]
#[template(path = "failed_game.html")]
struct GameFailedPage {
    id: Uuid,
}

#[get("/game/<id>/debug")]
pub(crate) async fn debug(db: Db, id: Uuid) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_debug(db, id).await? {
        Some(log) => Some(Html(
            GameDebugPage { id, log }
                .render()
                .map_err(anyhow::Error::from)?,
        )),
        None => None,
    })
}

async fn load_debug(db: Db, id: Uuid) -> Result<Option<Vec<LogEntry>>> {
    let data: Option<Vec<u8>> = db
        .run(move |conn| {
            conn.query_row(
                "SELECT log_json_zst FROM game_debug WHERE game_id = ?",
                &[&id as &dyn ToSql],
                |row| row.get(0),
            )
            .optional()
        })
        .await?;
    Ok(match data {
        Some(data) => Some(serde_json::from_slice(&zstd::decode_all(&*data)?)?),
        None => None,
    })
}

impl LogEntry {
    fn description(&self) -> &str {
        match self {
            LogEntry::Ok { description, .. } => description,
            LogEntry::Err { description, .. } => description.as_deref().unwrap_or("[end of feed]"),
        }
    }

    fn info(&self) -> Cow<'_, str> {
        match self {
            LogEntry::Ok { patch, .. } => patch
                .0
                .iter()
                .map(|p| {
                    serde_json::to_string(p)
                        .unwrap_or_else(|_| "[failed to serialize patch]".to_string())
                })
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
            LogEntry::Err { error, .. } => error.into(),
        }
    }
}

#[derive(Template)]
#[template(path = "debug_game.html")]
struct GameDebugPage {
    id: Uuid,
    log: Vec<LogEntry>,
}
