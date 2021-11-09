use crate::{cache, API_BASE, CLIENT};
use anyhow::Result;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use uuid::Uuid;

const CACHE_KIND: &str = "Schedule";

type ScheduleResponse = BTreeMap<u16, Box<RawValue>>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    id: Uuid,
    game_complete: bool,
}

fn schedule_to_ids(schedule: Vec<Game>) -> Vec<Uuid> {
    schedule
        .into_iter()
        .filter(|g| g.game_complete)
        .map(|g| g.id)
        .collect()
}

fn cache_key(sim: &str, season: u16, day: u16) -> String {
    format!("{},{},{}", sim, season, day)
}

pub(crate) async fn load_schedule(
    sim: &str,
    season: u16,
    start_day: u16,
    end_day: u16,
) -> Result<Vec<Uuid>> {
    let day_range = start_day..=end_day;

    let mut cached: BTreeMap<u16, Vec<Uuid>> = BTreeMap::new();
    for day in day_range.clone() {
        if let Some(value) = cache::load(CACHE_KIND, &cache_key(sim, season, day), None).await? {
            cached.insert(day, schedule_to_ids(serde_json::from_slice(&value)?));
        }
    }

    if let Some(min_missing) = day_range.clone().find(|day| !cached.contains_key(day)) {
        let max_missing = day_range
            .clone()
            .rev()
            .find(|day| !cached.contains_key(day))
            .unwrap();
        let response: ScheduleResponse = CLIENT
            .get(format!(
                "{}/api/games/schedule?sim={}&season={}&startDay={}&endDay={}",
                API_BASE, sim, season, min_missing, max_missing
            ))
            .send()
            .await?
            .json()
            .await?;
        for (day, raw_schedule) in response {
            let schedule: Vec<Game> = serde_json::from_str(raw_schedule.get())?;
            if schedule.iter().all(|game| game.game_complete) {
                cache::store(
                    CACHE_KIND,
                    &cache_key(sim, season, day),
                    raw_schedule.get().as_bytes(),
                    None,
                )
                .await?;
            }
            cached.insert(day, schedule_to_ids(schedule));
        }
    }

    Ok(cached.into_values().flatten().collect())
}
