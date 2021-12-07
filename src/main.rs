mod batting;
mod chronicler;
mod debug;
mod feed;
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

const API_BASE: &str = "https://api.blaseball.com";
const CHRONICLER_BASE: &str = "https://api.sibr.dev/chronicler";
const CONFIGS_BASE: &str = "https://blaseball-configs.s3.us-west-2.amazonaws.com";
const SACHET_BASE: &str = "https://api.sibr.dev/eventually/sachet";

static REBUILDING: AtomicBool = AtomicBool::new(false);

// Used to mark certain schema changes that require reprocessing all games.
const DB_MARKERS: &[&str] = &[
    "marker_stats_games_played",
    "marker_combined_names",
    "marker_split_games_played",
    "marker_clear_on_marker",
    "marker_force_emoji_representation",
    "marker_summary_first_day",
    "marker_shutouts_fixed",
    "marker_postseason_99_fix",
    "marker_common_names_tree",
    "marker_recorded_seasons_tree",
    "marker_all_team_summaries_v3",
    "marker_test_rebuild",
];
const CLEAR_ON_MARKER: &[&str] = &[summary::TREE, summary::SEASON_TREE];
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
    let mut force = false;
    for marker in DB_MARKERS.iter().rev() {
        if !DB.contains_key(marker)? {
            force = true;
            REBUILDING.store(true, Ordering::Relaxed);
            for tree in CLEAR_ON_MARKER {
                DB.drop_tree(tree)?;
            }
            break;
        }
    }

    seasons::load().await?;

    for season in Season::iter_known()? {
        let season = season?;
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
    lazy_static::initialize(&DB);

    rocket::build()
        .mount(
            "/",
            routes![
                routes::brick,
                routes::css,
                routes::debug::debug,
                routes::debug::errors,
                routes::game::game,
                routes::jump,
                routes::player::player,
                routes::team::team,
                routes::tablesort,
                routes::tablesort_number,
                routes::season::season_batting,
                routes::season::season_pitching,
                routes::index,
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
                        const NEEDLE: &str = "<p data-rebuild class=\"hidden";
                        const UNHIDE: &[u8] = b"<p data-rebuild class=\"      ";
                        const _: () = assert!(NEEDLE.len() == UNHIDE.len());

                        let rebuild_pos = html.find(NEEDLE);
                        let mut html = html.into_bytes();

                        if REBUILDING.load(Ordering::Relaxed) {
                            if let Some(pos) = rebuild_pos {
                                (&mut html[pos..(pos + UNHIDE.len())]).copy_from_slice(UNHIDE);
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
