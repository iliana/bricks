mod feed;
mod team;
mod game;
mod stats;

use anyhow::Result;
use reqwest::Client;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

const API_BASE: &str = "https://api.blaseball.com";

lazy_static::lazy_static! {
    static ref CLIENT: Client = Client::new();
    static ref DB: Arc<Mutex<Connection>> = {
        let path = std::env::var("BRICKS_DB").expect("BRICKS_DB environment variable not set");
        Arc::new(Mutex::new(Connection::open(path).expect("failed to open database")))
    };
}

refinery::embed_migrations!("./migrations");

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    migrations::runner().run(&mut *DB.lock().await)?;

    let feed = feed::load_game_feed("781beeba-1890-4526-9dd5-094844ac884e").await?;
    println!("{:#?}", game::process_game(&feed).await?);

    Ok(())
}
