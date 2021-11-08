mod cache;
mod chronicler;
mod feed;
mod filters;
mod game;
mod stats;
mod team;

use crate::stats::{AwayHome, GameStats};
use anyhow::Result;
use askama::Template;
use reqwest::Client;
use rusqlite::Connection;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const CHRONICLER_BASE: &str = "https://api.sibr.dev/chronicler";
const SACHET_BASE: &str = "https://api.sibr.dev/eventually/sachet";

lazy_static::lazy_static! {
    static ref CLIENT: Client = Client::builder()
        .user_agent("bricks/0.0 (iliana@sibr.dev)")
        .build()
        .unwrap();

    static ref DB: Arc<Mutex<Connection>> = {
        let path = std::env::var("BRICKS_DB").expect("BRICKS_DB environment variable not set");
        Arc::new(Mutex::new(Connection::open(path).expect("failed to open database")))
    };
}

refinery::embed_migrations!("./migrations");

#[derive(Template)]
#[template(path = "game.html")]
struct GamePage {
    stats: AwayHome<GameStats>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    migrations::runner().run(&mut *DB.lock().await)?;

    let out_dir =
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_else(|| OsStr::new("out").into()));

    let feed = feed::load_game_feed("3b63f242-8590-4bf0-a2d7-884edb0b2e90").await?;
    let mut state = game::State::new();
    for event in &feed {
        state.push(event).await?;
    }
    tokio::fs::create_dir_all(out_dir.join("game")).await?;
    tokio::fs::write(
        out_dir
            .join("game")
            .join("3b63f242-8590-4bf0-a2d7-884edb0b2e90.html"),
        GamePage {
            stats: state.finish()?,
        }
        .render()?,
    )
    .await?;

    Ok(())
}
