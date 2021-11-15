mod chronicler;
mod debug;
mod feed;
mod game;
mod names;
mod percentage;
mod routes;
mod schedule;
mod seasons;
mod state;
mod summary;
mod table;
mod team;

use anyhow::{bail, Result};
use reqwest::Client;
use rocket::fairing::AdHoc;
use rocket::http::ContentType;
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

// Used to mark certain schema changes that require reprocessing all games.
const DB_MARKERS: &[&str] = &[
    "marker_stats_games_played",
    "marker_combined_names",
    "marker_split_games_played",
    "marker_clear_on_marker",
];
const CLEAR_ON_MARKER: &[&str] = &[summary::TREE];
const OLD_TREES: &[&str] = &[
    "game_stats_v1",
    "game_stats_v2",
    "player_names_v1",
    "player_stats_v1",
    "player_stats_v2",
    "player_stats_v3",
];

lazy_static::lazy_static! {
    static ref DB: Db = sled::Config::default()
        .path(std::env::var_os("BRICKS_SLED_V1").expect("BRICKS_SLED_V1 not set in environment"))
        .use_compression(true)
        .open()
        .unwrap();
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

async fn process_game_or_log(sim: &str, id: Uuid, force: bool) {
    match game::process(sim, id, force).await {
        Ok(true) => log::info!("processed game {}", id),
        Ok(false) => {}
        Err(err) => log::error!("failed to process game {}: {:#}", id, err),
    }
}

async fn start_task() -> Result<()> {
    let mut force = false;
    for marker in DB_MARKERS {
        if !DB.contains_key(marker)? {
            force = true;
            for tree in CLEAR_ON_MARKER {
                DB.drop_tree(tree)?;
            }
            break;
        }
    }

    seasons::load().await?;

    for season in seasons::iter()? {
        let (sim, season) = season?;
        if sim == "thisidisstaticyo" || sim == "gamma4" {
            continue;
        }
        if let Some(last_day) = schedule::last_day(&sim, season).await? {
            for game_id in schedule::load(&sim, season, 0, last_day).await? {
                process_game_or_log(&sim, game_id, force).await;
            }
        } else {
            bail!("failed to get last day for sim {} season {}", sim, season);
        }
    }

    for marker in DB_MARKERS {
        DB.insert(marker, "")?;
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
        process_game_or_log(&now.sim, game_id, false).await;
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
                routes::player::player,
            ],
        )
        .attach(AdHoc::on_liftoff("Background tasks", |_rocket| {
            Box::pin(async {
                if std::env::var_os("DISABLE_TASKS").is_none() {
                    tokio::spawn(async {
                        log_err!(start_task().await);
                        loop {
                            sleep(Duration::from_secs(120)).await;
                            log_err!(update_task().await);
                        }
                    });
                }

                tokio::spawn(async {
                    for tree in OLD_TREES {
                        log_err!(DB.drop_tree(tree));
                    }
                });
            })
        }))
        .attach(AdHoc::on_response("HTML minifier", |_, response| {
            Box::pin(async move {
                if response.content_type() == Some(ContentType::HTML) {
                    if let Ok(html) = response.body_mut().take().to_bytes().await {
                        let mini = minify_html::minify(
                            &html,
                            &minify_html::Cfg {
                                keep_closing_tags: true,
                                keep_html_and_head_opening_tags: true,
                                ..minify_html::Cfg::spec_compliant()
                            },
                        );
                        response.set_sized_body(mini.len(), std::io::Cursor::new(mini));
                    }
                }
            })
        }))
}
