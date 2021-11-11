mod chronicler;
mod db;
mod feed;
mod filters;
mod game;
mod names;
mod schedule;
mod seasons;
mod stats;
mod team;
mod today;

use crate::db::Db;
use crate::seasons::Season;
use anyhow::Context;
use reqwest::Client;
use rocket::fairing::AdHoc;
use rocket::http::ContentType;
use rocket::tokio::time::sleep;
use rocket::{get, launch, routes, tokio, Orbit, Rocket};
use std::time::Duration;

type ResponseResult<T> = std::result::Result<T, rocket::response::Debug<anyhow::Error>>;

const API_BASE: &str = "https://api.blaseball.com";
const CONFIGS_BASE: &str = "https://blaseball-configs.s3.us-west-2.amazonaws.com";
const CHRONICLER_BASE: &str = "https://api.sibr.dev/chronicler";
const SACHET_BASE: &str = "https://api.sibr.dev/eventually/sachet";

lazy_static::lazy_static! {
    static ref CLIENT: Client = Client::builder()
        .user_agent("bricks/0.0 (iliana@sibr.dev)")
        .build()
        .unwrap();
}

trait ResultExt<T, E> {
    fn log_err(self) -> Option<T>;
}

impl<T, E: std::fmt::Debug> ResultExt<T, E> for Result<T, E> {
    fn log_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(err) => {
                log::error!("{:?}", err);
                None
            }
        }
    }
}

#[get("/styles.css")]
fn css() -> (ContentType, &'static [u8]) {
    (
        ContentType::CSS,
        include_bytes!(concat!(env!("OUT_DIR"), "/styles.css")),
    )
}

async fn background(rocket: &Rocket<Orbit>) {
    let db = Db::get_one(rocket).await.unwrap();
    tokio::spawn(async move {
        async fn process_season(db: &Db, season: Season, last_day_only: bool) {
            let start = if last_day_only {
                season.last_day.max(1) - 1
            } else {
                0
            };

            if let Some(schedule) =
                schedule::load_schedule(db, &season.sim, season.season, start, season.last_day)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to load schedule for {} season {}",
                            season.sim, season.season
                        )
                    })
                    .log_err()
            {
                for (day, game) in schedule {
                    if !game::is_done(db, game).await.log_err().unwrap_or_default() {
                        match game::process_game(db, &season.sim, season.season, day, game).await {
                            Ok(()) => log::info!("processed game {}", game),
                            Err(_) => log::warn!("failed to process game {}", game),
                        }
                    }
                }
            }
        }

        // load all past games
        if let Some(seasons) = seasons::load_seasons().await.log_err() {
            for season in seasons {
                process_season(&db, season, false).await;
            }
        }

        // loop for loading new games
        loop {
            if let Some(today) = today::load_today()
                .await
                .context("failed to fetch simulationData")
                .log_err()
            {
                process_season(
                    &db,
                    Season {
                        sim: today.id,
                        season: today.season,
                        last_day: today.day,
                    },
                    true,
                )
                .await;
            }

            sleep(Duration::from_secs(120)).await;
        }
    });
}

#[launch]
fn rocket() -> _ {
    let mut rocket = rocket::build();
    if std::env::var_os("DISABLE_TASKS").is_none() {
        rocket = rocket.attach(AdHoc::on_liftoff("Background tasks", |rocket| {
            Box::pin(background(rocket))
        }))
    }
    rocket
        .mount(
            "/",
            routes![
                css,
                game::errors::errors,
                game::routes::debug,
                game::routes::game,
            ],
        )
        .attach(Db::fairing())
        .attach(AdHoc::try_on_ignite(
            "Database migrations",
            db::run_migrations,
        ))
}
