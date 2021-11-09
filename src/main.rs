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
use uuid::Uuid;

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

    static ref OUT_DIR: PathBuf = PathBuf::from(
        std::env::var_os("OUT_DIR").unwrap_or_else(|| OsStr::new("out").into())
    );
}

refinery::embed_migrations!("./migrations");

#[derive(Template)]
#[template(path = "game.html")]
struct GamePage {
    stats: AwayHome<GameStats>,
}

async fn render_game(id: Uuid) -> Result<()> {
    let feed = feed::load_game_feed(id).await?;
    let mut state = game::State::new();
    for event in &feed {
        state.push(event).await?;
    }
    tokio::fs::create_dir_all(OUT_DIR.join("game")).await?;
    tokio::fs::write(
        OUT_DIR.join("game").join(format!("{}.html", id)),
        GamePage {
            stats: state.finish()?,
        }
        .render()?,
    )
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    migrations::runner().run(&mut *DB.lock().await)?;

    render_game("cf71a46c-d7f3-4ec3-80ed-2888bac0a22e".parse()?).await?;

    Ok(())
}
