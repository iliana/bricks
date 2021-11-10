pub(crate) mod routes;
pub(crate) mod state;

use crate::db::Db;
use anyhow::Result;
use json_patch::Patch;
use rusqlite::{OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub(crate) async fn is_done(db: &Db, id: Uuid) -> Result<bool> {
    Ok(db
        .run(move |conn| {
            conn.query_row(
                "SELECT game_id FROM game_stats WHERE game_id = ? AND stats_json_zst IS NOT NULL",
                &[&id as &dyn ToSql],
                |_| Ok(()),
            )
            .optional()
        })
        .await?
        .is_some())
}

pub(crate) async fn process_game(
    db: &Db,
    sim: &str,
    season: u16,
    day: u16,
    id: Uuid,
) -> Result<()> {
    let feed = crate::feed::load_game_feed(db, id).await?;
    let mut state = state::State::new(db);
    let mut old = Value::default();
    let mut debug_log = Vec::new();

    let mut row = GameStatsRow {
        id,
        sim,
        season,
        day,
        away: Uuid::default(),
        home: Uuid::default(),
        stats: None,
    };

    for event in feed {
        match state.push(&event).await {
            Ok(()) => {
                let new = serde_json::to_value(&state)?;
                debug_log.push(LogEntry::Ok {
                    description: event.description,
                    patch: json_patch::diff(&old, &new),
                });
                old = new;
            }
            Err(err) => {
                debug_log.push(LogEntry::Err {
                    description: Some(event.description),
                    error: format!("{:?}", err),
                });
                store_debug_log(db, id, debug_log).await?;
                row.away = state.stats.away.team;
                row.home = state.stats.home.team;
                store_game_stats(db, row).await?;
                return Err(err);
            }
        }
    }

    row.away = state.stats.away.team;
    row.home = state.stats.home.team;

    let stats = match state.finish() {
        Ok(stats) => stats,
        Err(err) => {
            debug_log.push(LogEntry::Err {
                description: None,
                error: format!("{:?}", err),
            });
            store_debug_log(db, id, debug_log).await?;
            store_game_stats(db, row).await?;
            return Err(err);
        }
    };

    store_debug_log(db, id, debug_log).await?;
    row.stats = Some(serde_json::to_string(&stats)?);

    for team in stats {
        for (player_id, player_stats) in team.stats {
            let player_stats = serde_json::to_string(&player_stats)?;
            let sim_owned = sim.to_owned();
            db.run(move |conn| {
                conn.execute(
                    "INSERT INTO player_stats \
                        (game_id, team_id, player_id, sim, season, day, stats_json) \
                        VALUES (:game_id, :team_id, :player_id, :sim, :season, :day, :stats) \
                        ON CONFLICT (game_id, team_id, player_id) \
                        DO UPDATE SET stats_json = :stats",
                    &[
                        (":game_id", &id as &dyn ToSql),
                        (":team_id", &team.team),
                        (":player_id", &player_id),
                        (":sim", &sim_owned),
                        (":season", &season),
                        (":day", &day),
                        (":stats", &player_stats),
                    ],
                )
            })
            .await?;
        }
    }

    // do this last so it only works if the above doesn't error
    store_game_stats(db, row).await?;

    Ok(())
}

#[derive(Debug)]
struct GameStatsRow<'a> {
    id: Uuid,
    sim: &'a str,
    season: u16,
    day: u16,
    away: Uuid,
    home: Uuid,
    stats: Option<String>,
}

async fn store_game_stats(db: &Db, row: GameStatsRow<'_>) -> Result<()> {
    let sim = row.sim.to_owned();
    let stats = match row.stats {
        Some(stats) => Some(zstd::encode_all(stats.as_bytes(), 0)?),
        None => None,
    };
    let away = (row.away != Uuid::default()).then(|| row.away);
    let home = (row.home != Uuid::default()).then(|| row.home);
    db.run(move |conn| {
        conn.execute(
            "INSERT INTO game_stats (game_id, sim, season, day, away, home, stats_json_zst) \
                VALUES (:id, :sim, :season, :day, :away, :home, :stats) \
                ON CONFLICT (game_id) DO UPDATE \
                SET home = :home, away = :away, stats_json_zst = :stats",
            &[
                (":id", &row.id as &dyn ToSql),
                (":sim", &sim),
                (":season", &row.season),
                (":day", &row.day),
                (":away", &away),
                (":home", &home),
                (":stats", &stats),
            ],
        )
    })
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum LogEntry {
    Ok {
        description: String,
        patch: Patch,
    },
    Err {
        description: Option<String>,
        error: String,
    },
}

async fn store_debug_log(db: &Db, id: Uuid, log: Vec<LogEntry>) -> Result<()> {
    let compressed = zstd::encode_all(&*serde_json::to_vec(&log)?, 0)?;
    db.run(move |conn| {
        conn.execute(
            "INSERT INTO game_debug (game_id, log_json) VALUES (:id, :log) \
                ON CONFLICT (game_id) DO UPDATE SET log_json = :log",
            &[(":id", &id as &dyn ToSql), (":log", &compressed)],
        )
    })
    .await?;
    Ok(())
}
