use crate::stats::{AwayHome, GameStats};
use crate::{filters, OUT_DIR};
use anyhow::Result;
use askama::Template;
use serde_json::Value;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "game.html")]
struct GamePage {
    stats: AwayHome<GameStats>,
}

#[derive(Template)]
#[template(path = "failed_game.html")]
struct GameFailedPage {
    id: Uuid,
}

#[derive(Template)]
#[template(path = "debug_game.html")]
struct GameDebugPage {
    id: Uuid,
    log: Vec<(String, String)>,
}

pub(crate) async fn render_game(id: Uuid) -> Result<()> {
    let feed = crate::feed::load_game_feed(id).await?;
    let mut state = crate::game::State::new();
    let mut old = Value::default();
    let mut debug = GameDebugPage {
        id,
        log: Vec::new(),
    };
    let mut failed = false;
    for event in feed {
        match state.push(&event).await {
            Ok(()) => {
                let new = serde_json::to_value(&state)?;
                let patch = json_patch::diff(&old, &new)
                    .0
                    .into_iter()
                    .map(|p| serde_json::to_string_pretty(&p))
                    .collect::<serde_json::Result<Vec<_>>>()?;
                debug.log.push((event.description, patch.join("\n")));
                old = new;
            }
            Err(err) => {
                debug.log.push((event.description, format!("{:?}", err)));
                failed = true;
                break;
            }
        }
    }
    tokio::fs::create_dir_all(OUT_DIR.join("game")).await?;
    match state.finish() {
        Ok(stats) => {
            tokio::fs::write(
                OUT_DIR.join("game").join(format!("{}.html", id)),
                GamePage { stats }.render()?,
            )
            .await?;
        }
        Err(err) => {
            debug
                .log
                .push(("[end of feed]".into(), format!("{:?}", err)));
            failed = true;
        }
    }
    if failed {
        tokio::fs::write(
            OUT_DIR.join("game").join(format!("{}.html", id)),
            GameFailedPage { id }.render()?,
        )
        .await?;
    }
    tokio::fs::create_dir_all(OUT_DIR.join("game").join(id.to_string())).await?;
    tokio::fs::write(
        OUT_DIR.join("game").join(id.to_string()).join("debug.html"),
        debug.render()?,
    )
    .await?;

    Ok(())
}
