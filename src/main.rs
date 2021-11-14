mod chronicler;
mod debug;
mod feed;
mod game;
mod names;
mod routes;
mod schedule;
mod seasons;
mod state;
mod table;
mod team;

use anyhow::{bail, Context, Result};
use reqwest::Client;
use rocket::fairing::AdHoc;
use rocket::tokio::time::sleep;
use rocket::{launch, routes, tokio};
use serde::Deserialize;
use sled::Db;
use std::time::Duration;
use uuid::Uuid;

const API_BASE: &str = "https://api.blaseball.com";
const CHRONICLER_BASE: &str = "https://api.sibr.dev/chronicler";
const CONFIGS_BASE: &str = "https://blaseball-configs.s3.us-west-2.amazonaws.com";
const SACHET_BASE: &str = "https://api.sibr.dev/eventually/sachet";

lazy_static::lazy_static! {
    static ref DB: Db = open_db().unwrap();
    static ref CLIENT: Client = Client::builder()
        .user_agent("bricks/0.0 (iliana@sibr.dev)")
        .build()
        .unwrap();
}

macro_rules! log_err {
    ($expr:expr) => {
        match $expr {
            Ok(v) => Some(v),
            Err(err) => {
                log::error!("{:#}", err);
                None
            }
        }
    };
}

fn open_db() -> Result<Db> {
    let db = sled::Config::default()
        .path(std::env::var_os("BRICKS_SLED_V1").context("BRICKS_SLED_V1 not set in environment")?)
        .use_compression(true)
        .open()?;
    db.drop_tree("game_stats_v1")?;
    db.drop_tree("player_stats_v1")?;
    db.drop_tree("game_stats_v2")?;
    db.drop_tree("player_stats_v2")?;
    Ok(db)
}

async fn process_game_or_log(sim: &str, id: Uuid) {
    match game::process(sim, id).await {
        Ok(()) => log::info!("processed game {}", id),
        Err(err) => log::error!("failed to process game {}: {:#}", id, err),
    }
}

async fn start_task() -> Result<()> {
    seasons::load().await?;

    for season in seasons::iter()? {
        let (sim, season) = season?;
        if sim == "thisidisstaticyo" || sim == "gamma4" {
            continue;
        }
        if let Some(last_day) = schedule::last_day(&sim, season).await? {
            for game_id in schedule::load(&sim, season, 0, last_day).await? {
                process_game_or_log(&sim, game_id).await;
            }
        } else {
            bail!("failed to get last day for sim {} season {}", sim, season);
        }
    }

    Ok(())
}

async fn update_task() -> Result<()> {
    #[derive(Debug, Deserialize)]
    struct SimData {
        #[serde(rename = "id")]
        sim: String,
        season: u16,
        day: u16,
    }

    let now: SimData = CLIENT
        .get(format!("{}/database/simulationData", API_BASE))
        .send()
        .await?
        .json()
        .await?;
    if seasons::era_name(&now.sim, now.season)?.is_none() {
        seasons::load().await?;
    }

    for game_id in schedule::load(&now.sim, now.season, now.day.max(1) - 1, now.day).await? {
        process_game_or_log(&now.sim, game_id).await;
    }

    Ok(())
}

#[launch]
fn rocket() -> _ {
    dotenv::dotenv().ok();
    lazy_static::initialize(&DB);

    rocket::build()
        .mount(
            "/",
            routes![
                routes::css,
                routes::debug::debug,
                routes::debug::errors,
                routes::game::game,
            ],
        )
        .attach(AdHoc::on_liftoff("Background task", |_rocket| {
            Box::pin(async {
                if std::env::var_os("DISABLE_TASKS").is_none() {
                    tokio::spawn(async {
                        log_err!(start_task().await);
                        loop {
                            log_err!(update_task().await);
                            sleep(Duration::from_secs(120)).await;
                        }
                    });
                }
            })
        }))
}
