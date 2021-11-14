use crate::{debug::LogEntry, state::State, DB};
use anyhow::Result;
use derive_more::{Add, AddAssign, Sum};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sled::transaction::{ConflictableTransactionError, Transactional};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

pub const DEBUG_TREE: &str = "debug_v1";
pub const GAME_STATS_TREE: &str = "game_stats_v3";
pub const PLAYER_STATS_TREE: &str = "player_stats_v3";

pub async fn process(sim: &str, id: Uuid) -> Result<()> {
    let game_stats_tree = DB.open_tree(GAME_STATS_TREE)?;
    if !game_stats_tree.contains_key(id.as_bytes())? {
        let debug_tree = DB.open_tree(DEBUG_TREE)?;
        let player_stats_tree = DB.open_tree(PLAYER_STATS_TREE)?;
        let player_name_tree = DB.open_tree("player_names_v1")?;

        let mut state = State::new(sim);
        let mut debug_log = Vec::new();
        let mut old = Value::default();
        let mut feed = crate::feed::load(id).await?;
        feed.sort_unstable();
        for event in feed {
            match state.push(&event).await {
                Ok(()) => {
                    let new = serde_json::to_value(&state)?;
                    debug_log.push(LogEntry::Ok {
                        description: event.description,
                        patch: json_patch::diff(&old, &new),
                    });
                    old = new;
                }
                Err(err) => {
                    debug_log.push(LogEntry::Err {
                        description: Some(event.description),
                        error: format!("{:?}", err),
                    });
                    debug_tree.insert(id.as_bytes(), serde_json::to_vec(&debug_log)?.as_slice())?;
                    return Err(err);
                }
            }
        }
        let game = match state.finish() {
            Ok(game) => game,
            Err(err) => {
                debug_log.push(LogEntry::Err {
                    description: None,
                    error: format!("{:?}", err),
                });
                debug_tree.insert(id.as_bytes(), serde_json::to_vec(&debug_log)?.as_slice())?;
                return Err(err);
            }
        };
        debug_tree.insert(id.as_bytes(), serde_json::to_vec(&debug_log)?.as_slice())?;

        (&game_stats_tree, &player_stats_tree, &player_name_tree).transaction(
            |(game_stats_tree, player_stats_tree, player_name_tree)| {
                for team in game.teams() {
                    for (id, name) in &team.player_names {
                        player_name_tree.insert(id.as_bytes(), name.as_bytes())?;
                    }

                    for (id, stats) in &team.stats {
                        for key in [
                            player_stats_key(team.id, *id, sim, game.season),
                            player_stats_key(*id, team.id, sim, game.season),
                        ] {
                            let new = match player_stats_tree.get(&key)? {
                                None => Stats::default(),
                                Some(value) => serde_json::from_slice(&value)
                                    .map_err(ConflictableTransactionError::Abort)?,
                            } + *stats;
                            player_stats_tree.insert(
                                key.as_slice(),
                                serde_json::to_vec(&new)
                                    .map_err(ConflictableTransactionError::Abort)?
                                    .as_slice(),
                            )?;
                        }
                    }
                }

                game_stats_tree.insert(
                    id.as_bytes(),
                    serde_json::to_vec(&game)
                        .map_err(ConflictableTransactionError::Abort)?
                        .as_slice(),
                )?;

                Ok(())
            },
        )?;
    }

    Ok(())
}

fn player_stats_key(a: Uuid, b: Uuid, sim: &str, season: u16) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(a.as_bytes());
    key.extend_from_slice(b.as_bytes());
    key.extend_from_slice(sim.as_bytes());
    key.extend_from_slice(&season.to_be_bytes());
    key
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Game {
    pub sim: String,
    pub season: u16,
    pub day: u16,
    pub away: Team,
    pub home: Team,
}

impl Game {
    pub fn teams(&self) -> impl Iterator<Item = &Team> {
        self.into_iter()
    }

    pub fn teams_mut(&mut self) -> impl Iterator<Item = &mut Team> {
        [&mut self.away, &mut self.home].into_iter()
    }
}

impl<'a> IntoIterator for &'a Game {
    type Item = &'a Team;
    type IntoIter = std::array::IntoIter<&'a Team, 2>;

    fn into_iter(self) -> Self::IntoIter {
        [&self.away, &self.home].into_iter()
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub nickname: String,
    pub shorthand: String,
    pub emoji: String,

    pub player_names: HashMap<Uuid, String>,
    pub lineup: Vec<Vec<Uuid>>,
    pub pitchers: Vec<Uuid>,

    pub stats: IndexMap<Uuid, Stats>,
    pub inning_runs: BTreeMap<u16, u16>,
    pub left_on_base: usize,
}

impl Team {
    pub fn runs(&self) -> u16 {
        self.inning_runs.values().sum()
    }

    pub fn hits(&self) -> u16 {
        self.stats.values().map(|s| s.hits()).sum()
    }

    pub fn positions_mut(&mut self) -> impl Iterator<Item = &mut Vec<Uuid>> {
        self.lineup.iter_mut().chain([&mut self.pitchers])
    }
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, Add, AddAssign, Sum)]
pub struct Stats {
    // Batting stats
    pub plate_appearances: u16,
    pub at_bats: u16,
    pub at_bats_with_risp: u16,
    pub hits_with_risp: u16,
    pub singles: u16,
    pub doubles: u16,
    pub triples: u16,
    pub home_runs: u16,
    pub runs: u16,
    pub runs_batted_in: u16,
    pub sacrifice_hits: u16,
    pub sacrifice_flies: u16,
    pub stolen_bases: u16,
    pub caught_stealing: u16,
    pub strike_outs: u16,
    pub double_plays_grounded_into: u16,
    pub walks: u16,
    pub left_on_base: usize,

    // Pitching stats
    pub batters_faced: u16,
    pub outs_recorded: u16,
    pub hits_allowed: u16,
    pub home_runs_allowed: u16,
    pub earned_runs: u16,
    pub struck_outs: u16,
    pub walks_issued: u16,
    pub strikes_pitched: u16,
    pub balls_pitched: u16,
    pub flyouts_pitched: u16,
    pub groundouts_pitched: u16,
}

impl Stats {
    pub fn hits(&self) -> u16 {
        self.singles + self.doubles + self.triples + self.home_runs
    }

    pub fn total_bases(&self) -> u16 {
        self.singles + 2 * self.doubles + 3 * self.triples + 4 * self.home_runs
    }

    pub fn innings_pitched(&self) -> String {
        format!("{}.{}", self.outs_recorded / 3, self.outs_recorded % 3)
    }

    pub fn pitches_strikes(&self) -> String {
        if self.strikes_pitched + self.balls_pitched > 0 {
            format!(
                "{}-{}",
                self.strikes_pitched + self.balls_pitched,
                self.strikes_pitched
            )
        } else {
            String::new()
        }
    }

    pub fn groundouts_flyouts(&self) -> String {
        if self.groundouts_pitched + self.flyouts_pitched > 0 {
            format!("{}-{}", self.groundouts_pitched, self.flyouts_pitched)
        } else {
            String::new()
        }
    }
}
