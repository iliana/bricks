mod batting;
mod chronicler;
mod csv;
mod debug;
mod export;
mod feed;
mod fraction;
mod game;
mod names;
mod percentage;
mod pitching;
mod routes;
mod schedule;
mod seasons;
mod state;
mod summary;
mod table;
mod team;

use crate::seasons::Season;
use anyhow::Result;
use reqwest::Client;
use rocket::fairing::AdHoc;
use rocket::http::ContentType;
use rocket::tokio::time::sleep;
use rocket::{launch, routes, tokio};
use serde::Deserialize;
use sled::Db;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use uuid::Uuid;

const GITHUB_SHA: Option<&str> = option_env!("GITHUB_SHA");

const API_BASE: &str = "https://api.blaseball.com";
const CHRONICLER_BASE: &str = "https://api.sibr.dev/chronicler";
const CONFIGS_BASE: &str = "https://blaseball-configs.s3.us-west-2.amazonaws.com";
const SACHET_BASE: &str = "https://api.sibr.dev/eventually/sachet";

static REBUILDING: AtomicBool = AtomicBool::new(false);

// Increment this if you need to force a rebuild.
const DB_VERSION: &[u8] = &[16];
const CLEAR_ON_REBUILD: &[&str] = &[summary::TREE, summary::SEASON_TREE];
const OLD_TREES: &[&str] = &[];

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

async fn process_game_or_log(season: Season, id: Uuid, force: bool) {
    let start = Instant::now();
    match game::process(season, id, force).await {
        Ok(true) => log::info!("processed game {} in {:?}", id, Instant::now() - start),
        Ok(false) => {}
        Err(err) => log::error!("failed to process game {}: {:#}", id, err),
    }
}

async fn start_task() -> Result<()> {
    let force = {
        let version = DB.get("version")?;
        if version.is_none() {
            DB.clear()?;
        }
        if version.as_ref().map_or(false, |v| v == DB_VERSION) {
            false
        } else {
            log::info!(
                "version {:?} != {:?}, rebuilding",
                version,
                Some(DB_VERSION)
            );
            REBUILDING.store(true, Ordering::Relaxed);
            for tree in CLEAR_ON_REBUILD {
                DB.drop_tree(tree)?;
            }
            true
        }
    };

    seasons::load().await?;

    for season in Season::known()? {
        if season.sim == "thisidisstaticyo" || season.sim == "gamma4" {
            continue;
        }
        if let Some(last_day) = schedule::last_day(&season).await? {
            for game_id in schedule::load(&season, 0, last_day).await? {
                process_game_or_log(season.clone(), game_id, force).await;
            }
        }
    }

    REBUILDING.store(false, Ordering::Relaxed);

    DB.insert("version", DB_VERSION)?;
    if force {
        log::info!("database rebuilt, version {:?}", DB_VERSION);
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
    let season = Season {
        sim: now.sim,
        season: now.season,
    };
    if season.era_name()?.is_none() {
        seasons::load().await?;
    }

    for game_id in schedule::load(&season, now.day.max(1) - 1, now.day).await? {
        process_game_or_log(season.clone(), game_id, false).await;
    }

    Ok(())
}

#[launch]
fn rocket() -> _ {
    dotenv::dotenv().ok();

    rocket::build()
        .mount(
            "/",
            routes![
                routes::brick,
                routes::css,
                routes::debug::debug,
                routes::debug::errors,
                routes::export::season_player_summary_csv,
                routes::export::season_player_summary_json,
                routes::export::season_team_summary_csv,
                routes::export::season_team_summary_json,
                routes::game::game,
                routes::glossary,
                routes::index,
                routes::jump,
                routes::player::player,
                routes::season::season_player_batting,
                routes::season::season_player_pitching,
                routes::season::season_team_batting,
                routes::season::season_team_pitching,
                routes::tablesort,
                routes::tablesort_number,
                routes::team::team,
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
                    if let Ok(html) = response.body_mut().take().to_string().await {
                        const HIDDEN: &str = "<p data-rebuild class=\"hidden";
                        const UNHIDE: &[u8] = b"<p data-rebuild class=\"      ";
                        const _: () = assert!(HIDDEN.len() == UNHIDE.len());

                        const COMMIT: &str = "@COMMIT@";

                        let rebuild_pos = html.find(HIDDEN);
                        let commit_pos = (html.find(COMMIT), html.rfind(COMMIT));
                        let mut html = html.into_bytes();

                        if REBUILDING.load(Ordering::Relaxed) {
                            if let Some(pos) = rebuild_pos {
                                (&mut html[pos..(pos + UNHIDE.len())]).copy_from_slice(UNHIDE);
                            }
                        }
                        if let Some(short) = GITHUB_SHA.and_then(|s| s.get(..8)) {
                            if let (Some(left), Some(right)) = commit_pos {
                                for pos in [left, right] {
                                    (&mut html[pos..(pos + COMMIT.len())])
                                        .copy_from_slice(short.as_bytes());
                                }
                            }
                        }
                        response.set_sized_body(html.len(), std::io::Cursor::new(html.clone()));

                        match minify_html_onepass::with_friendly_error(
                            &mut html,
                            &minify_html_onepass::Cfg {
                                minify_js: true,
                                minify_css: false,
                            },
                        ) {
                            Ok(len) => {
                                html.truncate(len);
                                response.set_sized_body(html.len(), std::io::Cursor::new(html));
                            }
                            Err(error) => log::error!("while minifying HTML: {:?}", error),
                        }
                    }
                }
            })
        }))
}
