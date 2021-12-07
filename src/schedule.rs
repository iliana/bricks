use crate::names::{self, TeamName};
use crate::{seasons::Season, API_BASE, CLIENT, DB};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::mem::size_of_val;
use uuid::Uuid;

const TREE: &str = "schedule_v1";

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
    pub home: bool,
    pub opponent: TeamName,
    pub won: bool,
    pub score: u16,
    pub opponent_score: u16,
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
        if entry.won {
            record.wins += 1;
        } else {
            record.losses += 1;
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

    let tree = DB.open_tree(TREE)?;
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

    let mut v = Vec::new();
    for (day, games) in cached {
        for game in games {
            for team in [game.away_team, game.home_team] {
                v.push(game.id);

                let mut key = Vec::with_capacity(
                    season.sim.len()
                        + size_of_val(&season.season)
                        + size_of_val(&team)
                        + size_of_val(&day),
                );
                key.extend_from_slice(season.sim.as_bytes());
                key.extend_from_slice(&season.season.to_ne_bytes());
                key.extend_from_slice(team.as_bytes());
                key.extend_from_slice(&day.to_be_bytes());

                let (opponent_id, opponent_score) = game.opponent(team);
                tree.insert(
                    key.as_slice(),
                    serde_json::to_vec(&Entry {
                        id: game.id,
                        day,
                        home: game.home_team == team,
                        opponent: names::team_name(opponent_id)?.unwrap_or_default(),
                        won: game.winner() == team,
                        score: game.score(team),
                        opponent_score,
                    })?
                    .as_slice(),
                )?;
            }
        }
    }
    Ok(v)
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
    away_team: Uuid,
    home_team: Uuid,
    away_score: u16,
    home_score: u16,
    game_complete: bool,
    winner: Option<Uuid>,
}

impl Game {
    fn winner(&self) -> Uuid {
        if let Some(winner) = self.winner {
            winner
        } else if self.home_score > self.away_score {
            self.home_team
        } else {
            self.away_team
        }
    }

    fn score(&self, team: Uuid) -> u16 {
        if team == self.home_team {
            self.home_score
        } else {
            self.away_score
        }
    }

    fn opponent(&self, team: Uuid) -> (Uuid, u16) {
        if team == self.home_team {
            (self.away_team, self.away_score)
        } else {
            (self.home_team, self.home_score)
        }
    }
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
