use crate::stats::{AwayHome, GameStats};
use crate::{filters, OUT_DIR};
use anyhow::Result;
use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "game.html")]
struct GamePage {
    stats: AwayHome<GameStats>,
}

pub(crate) async fn render_game(id: Uuid) -> Result<()> {
    let feed = crate::feed::load_game_feed(id).await?;
    let mut state = crate::game::State::new();
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
