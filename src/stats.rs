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
    pub(crate) left_on_base: u16,
    pub(crate) lineup: Vec<Vec<Uuid>>,
    pub(crate) pitchers: Vec<Uuid>,
    pub(crate) stats: HashMap<Uuid, Stats>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Stats {
    // Batting stats
    pub(crate) plate_appearances: u16,
    pub(crate) at_bats: u16,
    pub(crate) singles: u16,
    pub(crate) doubles: u16,
    pub(crate) triples: u16,
    pub(crate) stolen_bases: u16,
    pub(crate) left_on_base: u16,

    // Pitching stats
    pub(crate) outs_recorded: u16,
    pub(crate) hits_allowed: u16,
    pub(crate) strikes_pitched: u16,
    pub(crate) balls_pitched: u16,
}

impl Stats {
    #[allow(unused)]
    pub(crate) fn total_pitches(&self) -> u16 {
        self.strikes_pitched + self.balls_pitched
    }
}
