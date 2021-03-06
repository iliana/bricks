use crate::game::Kind;
use crate::names::TeamName;
use crate::{seasons::Season, API_BASE, CLIENT, DB};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::mem::size_of_val;
use uuid::Uuid;

pub const TREE: &str = "schedule_v1";

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Record {
    pub wins: u16, // ~technically~ non-losses
    pub losses: u16,
}

impl Record {
    pub fn diff(&self) -> i32 {
        i32::from(self.wins) - i32::from(self.losses)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub id: Uuid,
    pub day: u16,
    #[serde(default)]
    pub kind: Kind,
    pub home: bool,
    pub opponent: TeamName,
    pub won: bool,
    pub score: u16,
    pub opponent_score: u16,
}

impl Entry {
    pub fn is_special(&self) -> bool {
        self.kind == Kind::Special
    }
}

pub fn schedule(team: Uuid, season: &Season) -> Result<Vec<(Record, Entry)>> {
    let tree = DB.open_tree(TREE)?;
    let mut search_key =
        Vec::with_capacity(season.sim.len() + size_of_val(&season.season) + size_of_val(&team));
    search_key.extend_from_slice(season.sim.as_bytes());
    search_key.extend_from_slice(&season.season.to_ne_bytes());
    search_key.extend_from_slice(team.as_bytes());
    let mut v = Vec::new();
    let mut record = Record::default();
    for row in tree.scan_prefix(&search_key) {
        let (_, value) = row?;
        let entry: Entry = serde_json::from_slice(&value)?;
        if entry.kind != Kind::Special {
            if entry.won {
                record.wins += 1;
            } else {
                record.losses += 1;
            }
        }
        v.push((record, entry));
    }
    Ok(v)
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

pub async fn load(season: &Season, start_day: u16, end_day: u16) -> Result<Vec<Uuid>> {
    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Query<'a> {
        #[serde(flatten)]
        season: &'a Season,
        start_day: u16,
        end_day: u16,
    }

    let cache_tree = DB.open_tree("cache_schedule_v1")?;

    let mut cached: BTreeMap<u16, Vec<Game>> = BTreeMap::new();
    for day in start_day..=end_day {
        if let Some(value) = cache_tree.get(&build_cache_key(season, day))? {
            cached.insert(day, filter_complete(serde_json::from_slice(&value)?));
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
                cache_tree.insert(&build_cache_key(season, day), raw_schedule.get())?;
            }
            cached.insert(day, filter_complete(schedule));
        }
    }

    Ok(cached.values().flatten().map(|game| game.id).collect())
}

fn filter_complete(schedule: Vec<Game>) -> Vec<Game> {
    schedule
        .into_iter()
        .filter(|game| game.game_complete)
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Game {
    id: Uuid,
    game_complete: bool,
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

fn build_cache_key(season: &Season, day: u16) -> Vec<u8> {
    let mut key =
        Vec::with_capacity(season.sim.len() + size_of_val(&season.season) + size_of_val(&day));
    key.extend_from_slice(season.sim.as_bytes());
    key.extend_from_slice(&season.season.to_ne_bytes());
    key.extend_from_slice(&day.to_ne_bytes());
    key
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

pub async fn last_day(season: &Season) -> Result<Option<u16>> {
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
                sim: &season.sim,
                season_start: season.season,
                season_end: season.season,
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
