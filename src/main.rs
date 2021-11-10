mod chronicler;
mod db;
mod feed;
mod filters;
mod game;
mod schedule;
mod stats;
mod team;

use reqwest::Client;
use rocket::fairing::AdHoc;
use rocket::http::ContentType;
use rocket::{get, launch, routes, tokio, Orbit, Rocket};

type ResponseResult<T> = std::result::Result<T, rocket::response::Debug<anyhow::Error>>;

const API_BASE: &str = "https://api.blaseball.com";
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

impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn log_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(err) => {
                log::error!("{}", err);
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
    let db = db::Db::get_one(rocket).await.unwrap();
    tokio::spawn(async move {
        let sim = "gamma8";
        let season = 0;
        if let Some(schedule) = schedule::load_schedule(&db, sim, season, 0, 0)
            .await
            .log_err()
        {
            for (day, game) in schedule {
                if !game::is_done(&db, game).await.log_err().unwrap_or_default() {
                    match game::process_game(&db, sim, season, day, game).await {
                        Ok(()) => log::info!("processed game {}", game),
                        Err(_) => log::error!("failed to process game {}", game),
                    }
                }
            }
        }
    });
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![css, game::routes::game, game::routes::debug])
        .attach(db::Db::fairing())
        .attach(AdHoc::try_on_ignite(
            "Database migrations",
            db::run_migrations,
        ))
        .attach(AdHoc::on_liftoff("Background tasks", |rocket| {
            Box::pin(background(rocket))
        }))
}
