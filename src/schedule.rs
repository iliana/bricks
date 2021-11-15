use crate::{API_BASE, CLIENT, DB};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use uuid::Uuid;

pub async fn load(
    sim: &str,
    season: u16,
    start_day: u16,
    end_day: u16,
) -> Result<impl Iterator<Item = Uuid>> {
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Query<'a> {
        sim: &'a str,
        season: u16,
        start_day: u16,
        end_day: u16,
    }

    let tree = DB.open_tree("cache_schedule_v1")?;

    let mut cached: BTreeMap<u16, Vec<Uuid>> = BTreeMap::new();
    for day in start_day..=end_day {
        if let Some(value) = tree.get(&build_key(sim, season, day))? {
            cached.insert(day, schedule_to_ids(serde_json::from_slice(&value)?));
        }
    }

    if let Some(start_missing) = (start_day..=end_day).find(|day| !cached.contains_key(day)) {
        let end_missing = (start_day..=end_day)
            .rev()
            .find(|day| !cached.contains_key(day))
            .unwrap();
        let response: BTreeMap<u16, Box<RawValue>> = CLIENT
            .get(format!(
                "{}/api/games/schedule?{}",
                API_BASE,
                serde_urlencoded::to_string(&Query {
                    sim,
                    season,
                    start_day: start_missing,
                    end_day: end_missing
                })?
            ))
            .send()
            .await?
            .json()
            .await?;
        for (day, raw_schedule) in response {
            let schedule: Vec<Game> = serde_json::from_str(raw_schedule.get())?;
            if schedule.iter().all(|game| game.game_complete) {
                tree.insert(&build_key(sim, season, day), raw_schedule.get())?;
            }
            cached.insert(day, schedule_to_ids(schedule));
        }
    }

    Ok(cached.into_values().flatten())
}

fn schedule_to_ids(schedule: Vec<Game>) -> Vec<Uuid> {
    schedule
        .into_iter()
        .filter(|g| g.game_complete)
        .map(|g| g.id)
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    id: Uuid,
    game_complete: bool,
}

fn build_key(sim: &str, season: u16, day: u16) -> Vec<u8> {
    let mut key = Vec::with_capacity(sim.len() + 2 * std::mem::size_of::<u16>());
    key.extend_from_slice(sim.as_bytes());
    key.extend_from_slice(&season.to_ne_bytes());
    key.extend_from_slice(&day.to_ne_bytes());
    key
}

pub async fn last_day(sim: &str, season: u16) -> Result<Option<u16>> {
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Query<'a> {
        #[serde(rename = "type")]
        ty: u16,
        sim: &'a str,
        season_start: u16,
        season_end: u16,
        sort: u8,
        limit: u8,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FeedEvent {
        day: u16,
    }

    let response: Vec<FeedEvent> = CLIENT
        .get(format!(
            "{}/database/feed/global?{}",
            API_BASE,
            serde_urlencoded::to_string(&Query {
                ty: 11,
                sim,
                season_start: season,
                season_end: season,
                sort: 0,
                limit: 1,
            })?
        ))
        .send()
        .await?
        .json()
        .await?;
    Ok(response.into_iter().next().map(|event| event.day))
}
