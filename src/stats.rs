use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Default)]
pub(crate) struct AwayHome<T> {
    pub(crate) away: T,
    pub(crate) home: T,
}

impl<T> AwayHome<T> {
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        [&mut self.away, &mut self.home].into_iter()
    }
}

#[derive(Debug, Default)]
pub(crate) struct GameStats {
    pub(crate) team: Uuid,
    pub(crate) name: String,
    pub(crate) nickname: String,
    pub(crate) shorthand: String,
    pub(crate) emoji: String,
    pub(crate) player_names: HashMap<Uuid, String>,
    pub(crate) lineup: Vec<Vec<Uuid>>,
    pub(crate) pitchers: Vec<Uuid>,

    pub(crate) stats: HashMap<Uuid, Stats>,
    pub(crate) inning_run_totals: HashMap<u16, u16>,
    pub(crate) left_on_base: usize,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Stats {
    // Batting stats
    pub(crate) plate_appearances: u16,
    pub(crate) at_bats: u16,
    pub(crate) at_bats_with_risp: u16,
    pub(crate) hits_with_risp: u16,
    pub(crate) singles: u16,
    pub(crate) doubles: u16,
    pub(crate) triples: u16,
    pub(crate) home_runs: u16,
    pub(crate) runs: u16,
    pub(crate) runners_batted_in: u16,
    pub(crate) stolen_bases: u16,
    pub(crate) strike_outs: u16,
    pub(crate) double_plays_grounded_into: u16,
    pub(crate) walks: u16,
    pub(crate) left_on_base: usize,

    // Pitching stats
    pub(crate) outs_recorded: u16,
    pub(crate) hits_allowed: u16,
    pub(crate) home_runs_allowed: u16,
    pub(crate) earned_runs: u16,
    pub(crate) struck_outs: u16,
    pub(crate) walks_issued: u16,
    pub(crate) strikes_pitched: u16,
    pub(crate) balls_pitched: u16,
}

impl Stats {
    pub(crate) fn hits(&self) -> u16 {
        self.singles + self.doubles + self.triples + self.home_runs
    }

    pub(crate) fn total_bases(&self) -> u16 {
        self.singles + 2 * self.doubles + 3 * self.triples + 4 * self.home_runs
    }

    pub(crate) fn total_pitches(&self) -> u16 {
        self.strikes_pitched + self.balls_pitched
    }
}
