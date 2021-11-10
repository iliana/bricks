use crate::{API_BASE, CLIENT, CONFIGS_BASE};
use anyhow::{Context, Result};
use rocket::tokio::sync::RwLock;
use serde::Deserialize;
use std::collections::HashMap;

lazy_static::lazy_static! {
    pub(crate) static ref SIM_NAMES: RwLock<HashMap<String, String>> = RwLock::default();
}

#[derive(Debug, Deserialize)]
struct Response {
    collection: Vec<SeasonEntry>,
}

#[derive(Debug, Deserialize)]
struct SeasonEntry {
    name: String,
    sim: String,
    seasons: Vec<u16>,
}

#[derive(Debug, Deserialize)]
struct Event {
    day: u16,
}

#[derive(Debug)]
pub(crate) struct Season {
    pub(crate) sim: String,
    pub(crate) season: u16,
    pub(crate) last_day: u16,
}

pub(crate) async fn load_seasons() -> Result<Vec<Season>> {
    let response: Response = CLIENT
        .get(format!("{}/feed_season_list.json", CONFIGS_BASE))
        .send()
        .await?
        .json()
        .await?;

    let mut guard = SIM_NAMES.write().await;
    for entry in &response.collection {
        guard.insert(entry.sim.clone(), entry.name.clone());
    }
    drop(guard);

    let mut seasons = Vec::new();
    for entry in response.collection {
        if entry.sim == "thisidisstaticyo" || entry.sim == "gamma4" {
            continue;
        }
        for season in entry.seasons.into_iter().map(|season| season - 1) {
            let query = format!(
                "type=11&sim={}&seasonStart={}&seasonEnd={}&sort=0&limit=1",
                entry.sim, season, season
            );
            let response: Vec<Event> = CLIENT
                .get(format!("{}/database/feed/global?{}", API_BASE, query))
                .send()
                .await?
                .json()
                .await?;
            let last_day = response
                .into_iter()
                .next()
                .with_context(|| format!("no feed events for sim {} season {}", entry.sim, season))?
                .day;
            seasons.push(Season {
                sim: entry.sim.clone(),
                season,
                last_day,
            });
        }
    }

    Ok(seasons)
}
