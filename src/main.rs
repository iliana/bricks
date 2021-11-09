mod cache;
mod chronicler;
mod feed;
mod filters;
mod game;
mod render;
mod schedule;
mod stats;
mod team;

use anyhow::Result;
use reqwest::Client;
use rusqlite::Connection;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const API_BASE: &str = "https://api.blaseball.com";
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

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    migrations::runner().run(&mut *DB.lock().await)?;

    let mut errored = 0;
    for game in schedule::load_schedule("gamma8", 1, 0, 8).await? {
        if !render::render_game(game).await? {
            errored += 1;
        }
    }

    eprintln!("errored: {}", errored);

    Ok(())
}
