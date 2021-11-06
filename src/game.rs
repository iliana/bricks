use crate::feed::GameEvent;
use crate::stats::Stats;
use crate::team;
use anyhow::{ensure, Context, Result};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct AwayHome<T> {
    away: T,
    home: T,
}

impl<T> AwayHome<T> {
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        [&mut self.away, &mut self.home].into_iter()
    }
}

#[derive(Debug, Default)]
pub(crate) struct GameStats {
    team: String,
    lineup: Vec<Vec<String>>,
    pitchers: Vec<String>,
    stats: HashMap<String, Stats>,
}

pub(crate) async fn process_game(feed: &[GameEvent]) -> Result<AwayHome<GameStats>> {
    let mut stats: AwayHome<GameStats> = AwayHome::default();

    // load initial team rosters
    let start_event = feed
        .iter()
        .find(|event| event.ty == 0)
        .context("no start event")?;
    ensure!(
        start_event.team_tags.len() == 2,
        "invalid team tags for event type 0"
    );
    stats
        .iter_mut()
        .zip(start_event.team_tags.iter())
        .for_each(|(stats, team)| stats.team = team.into());

    for team in stats.iter_mut() {
        let data = team::load_team(&team.team, start_event.created).await?;
        team.lineup = data.lineup.into_iter().map(|x| vec![x]).collect();
    }

    Ok(stats)
}
