use crate::feed::GameEvent;
use crate::stats::Stats;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct AwayHome<T> {
    away: T,
    home: T,
}

#[derive(Debug, Default)]
pub(crate) struct GameStats {
    lineup: Vec<Vec<String>>,
    pitchers: Vec<String>,
    stats: HashMap<String, Stats>,
}

pub(crate) async fn process_game(feed: &[GameEvent]) -> Result<AwayHome<GameStats>> {
    let mut stats: AwayHome<GameStats> = AwayHome::default();

    // load initial team rosters

    unimplemented!();

    Ok(stats)
}
