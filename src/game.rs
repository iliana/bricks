use crate::names::{self, TeamName};
use crate::seasons::{self, Season};
use crate::{debug::LogEntry, percentage::Pct, state::State, summary, DB};
use anyhow::Result;
use derive_more::{Add, AddAssign, Sum};
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sled::transaction::{ConflictableTransactionError, Transactional};
use std::collections::{BTreeMap, HashMap};
use std::mem::size_of_val;
use uuid::Uuid;

lazy_static::lazy_static! {
    static ref DEBUG: bool = std::env::var("BRICKS_DEBUG")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(true);
}

pub const DEBUG_TREE: &str = "debug_v1";
pub const GAME_STATS_TREE: &str = "game_stats_v3";

pub async fn process(season: Season, id: Uuid, force: bool) -> Result<bool> {
    let game_stats_tree = DB.open_tree(GAME_STATS_TREE)?;
    if force || !game_stats_tree.contains_key(id.as_bytes())? {
        let debug_tree = DB.open_tree(DEBUG_TREE)?;
        let summary_tree = DB.open_tree(summary::TREE)?;
        let season_summary_tree = DB.open_tree(summary::SEASON_TREE)?;
        let names_tree = DB.open_tree(names::TREE)?;
        let common_names_tree = DB.open_tree(names::COMMON_TREE)?;
        let recorded_tree = DB.open_tree(seasons::RECORDED_TREE)?;

        let mut state = State::new(season);
        let mut debug_log = Vec::new();
        let mut old = Value::default();
        let feed = crate::feed::load(id).await?;
        for event in feed {
            match state.push(&event).await {
                Ok(()) => {
                    if *DEBUG {
                        let new = serde_json::to_value(&state)?;
                        debug_log.push(LogEntry::Ok {
                            description: event.description,
                            patch: json_patch::diff(&old, &new),
                        });
                        old = new;
                    }
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

        (
            &game_stats_tree,
            &summary_tree,
            &season_summary_tree,
            &names_tree,
            &common_names_tree,
            &recorded_tree,
        )
            .transaction(
                |(
                    game_stats_tree,
                    summary_tree,
                    season_summary_tree,
                    names_tree,
                    common_names_tree,
                    recorded_tree,
                )| {
                    for team in game.teams() {
                        names_tree.insert(
                            team.id.as_bytes(),
                            serde_json::to_vec(&team.name)
                                .map_err(ConflictableTransactionError::Abort)?,
                        )?;
                        for (id, name) in &team.player_names {
                            names_tree.insert(id.as_bytes(), name.as_bytes())?;
                        }

                        let mut common_key = Vec::new();
                        common_key.extend_from_slice(&team.name.emoji_hash().to_ne_bytes());
                        common_key.extend_from_slice(&game.season.season.to_ne_bytes());
                        common_key.extend_from_slice(game.season.sim.as_bytes());
                        common_names_tree.insert(common_key, team.id.as_bytes())?;
                    }

                    summary::write_summary(summary_tree, season_summary_tree, &game)?;

                    game_stats_tree.insert(
                        id.as_bytes(),
                        serde_json::to_vec(&game)
                            .map_err(ConflictableTransactionError::Abort)?
                            .as_slice(),
                    )?;

                    let mut key = Vec::with_capacity(
                        game.season.sim.len() + size_of_val(&game.season.season),
                    );
                    key.extend_from_slice(game.season.sim.as_bytes());
                    key.extend_from_slice(&game.season.season.to_be_bytes());
                    recorded_tree.insert(key, Vec::new())?;

                    Ok(())
                },
            )?;

        Ok(true)
    } else {
        Ok(false)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Game {
    #[serde(flatten)]
    pub season: Season,
    pub day: u16,
    pub is_postseason: bool,
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

    pub fn winner(&self) -> &Team {
        if self.away.won {
            &self.away
        } else {
            &self.home
        }
    }

    pub fn loser(&self) -> &Team {
        if self.away.won {
            &self.home
        } else {
            &self.away
        }
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
#[serde(default)]
pub struct Team {
    pub id: Uuid,
    #[serde(flatten)]
    pub name: TeamName,
    pub won: bool,

    pub player_names: HashMap<Uuid, String>,
    pub lineup: Vec<Vec<Uuid>>,
    pub pitchers: Vec<Uuid>,
    pub pitcher_of_record: Uuid,
    pub saving_pitcher: Option<Uuid>,

    pub stats: IndexMap<Uuid, Stats>,
    pub inning_runs: BTreeMap<u16, u16>,
    pub left_on_base: usize,

    pub crisp: IndexSet<Uuid>,
}

impl Team {
    pub fn runs(&self) -> u16 {
        self.inning_runs.values().sum()
    }

    pub fn hits(&self) -> u32 {
        self.stats.values().map(|s| s.hits()).sum()
    }

    pub fn positions_mut(&mut self) -> impl Iterator<Item = &mut Vec<Uuid>> {
        self.lineup.iter_mut().chain([&mut self.pitchers])
    }
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Deserialize,
    Serialize,
    Add,
    AddAssign,
    Sum,
)]
#[serde(default)]
pub struct Stats {
    pub games_batted: u32,
    pub games_pitched: u32,

    // Batting stats
    pub plate_appearances: u32,
    pub at_bats: u32,
    pub at_bats_with_risp: u32,
    pub hits_with_risp: u32,
    pub singles: u32,
    pub doubles: u32,
    pub triples: u32,
    pub home_runs: u32,
    pub runs: u32,
    pub runs_batted_in: u32,
    pub sacrifices: u32,
    pub stolen_bases: u32,
    pub caught_stealing: u32,
    pub strike_outs: u32,
    pub double_plays_grounded_into: u32,
    pub walks: u32,
    pub left_on_base: usize,

    // Pitching stats
    pub wins: u32,
    pub losses: u32,
    pub games_started: u32,
    pub games_finished: u32,
    pub complete_games: u32,
    pub shutouts: u32,
    pub no_hitters: u32,
    pub perfect_games: u32,
    pub saves: u32,
    pub batters_faced: u32,
    pub outs_recorded: u32,
    pub hits_allowed: u32,
    pub home_runs_allowed: u32,
    pub earned_runs: u32,
    pub struck_outs: u32,
    pub walks_issued: u32,
    pub strikes_pitched: u32,
    pub balls_pitched: u32,
    pub flyouts_pitched: u32,
    pub groundouts_pitched: u32,
}

impl Stats {
    pub fn is_batting(&self) -> bool {
        self.plate_appearances > 0
    }

    pub fn is_pitching(&self) -> bool {
        self.strikes_pitched + self.balls_pitched > 0
    }

    pub fn hits(&self) -> u32 {
        self.singles + self.doubles + self.triples + self.home_runs
    }

    pub fn total_bases(&self) -> u32 {
        self.singles + 2 * self.doubles + 3 * self.triples + 4 * self.home_runs
    }

    pub fn batting_average(&self) -> Pct<3> {
        Pct::new(self.hits(), self.at_bats)
    }

    pub fn on_base_percentage(&self) -> Pct<3> {
        Pct::new(
            self.hits() + self.walks,
            self.at_bats + self.walks + self.sacrifices,
        )
    }

    pub fn slugging_percentage(&self) -> Pct<3> {
        Pct::new(self.total_bases(), self.at_bats)
    }

    pub fn on_base_plus_slugging(&self) -> Pct<3> {
        Pct(self.on_base_percentage().0 + self.slugging_percentage().0)
    }

    pub fn batting_average_on_balls_in_play(&self) -> Pct<3> {
        Pct::new(
            self.hits() - self.home_runs,
            self.at_bats - self.strike_outs - self.home_runs + self.sacrifices,
        )
    }

    pub fn ops_plus(&self, league: Stats) -> Pct<0> {
        let obp = self.on_base_percentage().0 / league.on_base_percentage().0;
        let slg = self.slugging_percentage().0 / league.slugging_percentage().0;
        let pct = obp + slg - 1.into();
        Pct(pct * 100.into())
    }

    pub fn win_loss_percentage(&self) -> Pct<3> {
        Pct::new(self.wins, self.wins + self.losses)
    }

    pub fn earned_run_average(&self) -> Pct<2> {
        Pct::new(self.earned_runs * 27, self.outs_recorded)
    }

    pub fn innings_pitched(&self) -> String {
        format!("{}.{}", self.outs_recorded / 3, self.outs_recorded % 3)
    }

    pub fn whip(&self) -> Pct<3> {
        Pct::new(
            (self.walks_issued + self.hits_allowed) * 3,
            self.outs_recorded,
        )
    }

    pub fn hits_per_9(&self) -> Pct<1> {
        Pct::new(self.hits_allowed * 27, self.outs_recorded)
    }

    pub fn home_runs_per_9(&self) -> Pct<1> {
        Pct::new(self.home_runs_allowed * 27, self.outs_recorded)
    }

    pub fn walks_per_9(&self) -> Pct<1> {
        Pct::new(self.walks_issued * 27, self.outs_recorded)
    }

    pub fn struck_outs_per_9(&self) -> Pct<1> {
        Pct::new(self.struck_outs * 27, self.outs_recorded)
    }

    pub fn struck_outs_walks_ratio(&self) -> Pct<2> {
        Pct::new(self.struck_outs, self.walks_issued)
    }

    pub fn era_plus(&self, league: Stats) -> Pct<0> {
        let pct = league.earned_run_average().0 / self.earned_run_average().0;
        Pct(pct * 100.into())
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
