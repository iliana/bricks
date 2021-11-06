use crate::feed::GameEvent;
use crate::stats::Stats;
use crate::team;
use anyhow::{bail, ensure, Context, Result};
use std::collections::HashMap;

macro_rules! o {
    ($expr:expr) => {
        match $expr {
            Some(v) => v,
            None => bail!("missing field {}", stringify!($expr)),
        }
    };
}

#[derive(Debug, Default)]
pub(crate) struct AwayHome<T> {
    away: T,
    home: T,
}

impl<T> AwayHome<T> {
    fn offense_mut(&mut self, top_of_inning: bool) -> &mut T {
        if top_of_inning {
            &mut self.away
        } else {
            &mut self.home
        }
    }

    fn defense_mut(&mut self, top_of_inning: bool) -> &mut T {
        if top_of_inning {
            &mut self.home
        } else {
            &mut self.away
        }
    }

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

impl GameStats {
    fn pitcher(&self) -> Option<&str> {
        self.pitchers.last().map(String::as_str)
    }

    fn slots_mut(&mut self) -> impl Iterator<Item = &mut Vec<String>> {
        self.lineup.iter_mut().chain([&mut self.pitchers])
    }
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
        for player in data.lineup {
            team.lineup.push(vec![player]);
        }
    }

    // set initial pitchers
    feed.iter()
        .find_map(|event| {
            if event.ty == 0 {
                None
            } else {
                match (event.away_pitcher.as_ref(), event.home_pitcher.as_ref()) {
                    (Some(away), Some(home)) => Some([away, home]),
                    _ => None,
                }
            }
        })
        .context("no starting pitcher data")?
        .into_iter()
        .zip(stats.iter_mut())
        .for_each(|(player, stats)| {
            stats.pitchers.push(player.into());
        });

    let mut top_of_inning = false;
    let mut batter: Option<String> = None;
    let mut batters_on = 0_usize;

    for event in feed {
        macro_rules! ostat {
            ($player:expr) => {
                stats
                    .offense_mut(top_of_inning)
                    .stats
                    .entry($player.into())
                    .or_default()
            };
        }

        macro_rules! pstat {
            () => {{
                let d = stats.defense_mut(top_of_inning);
                let pitcher = o!(d.pitcher()).to_owned();
                d.stats.entry(pitcher).or_default()
            }};
        }

        match event.ty {
            2 => {
                // Half-inning
                top_of_inning = !top_of_inning;
                batter = None;
                batters_on = 0;
            }
            3 => {
                // Pitching change
                ensure!(
                    event.player_tags.len() == 1,
                    "invalid player tags for type 3"
                );
                stats
                    .defense_mut(top_of_inning)
                    .pitchers
                    .push((&event.player_tags[0]).into());
            }
            4 => {
                // Stolen base
                ensure!(
                    event.player_tags.len() == 1,
                    "invalid player tags for type 3"
                );
                ostat!(&event.player_tags[0]).stolen_bases += 1;
            }
            5 => {
                // Walk
                ensure!(
                    event.player_tags.len() == 1,
                    "invalid player tags for type 3"
                );
                ostat!(&event.player_tags[0]).walks += 1;
                pstat!().walks_issued += 1;
            }
            6 => {
                // Strikeout
                ensure!(
                    event.player_tags.len() == 1,
                    "invalid player tags for type 6"
                );
                ostat!(&event.player_tags[0]).struckouts += 1;
                let pitcher = pstat!();
                pitcher.outs_recorded += 1;
                pitcher.strikeouts += 1;
            }
            7 | 8 => {
                // Flyout or ground out.
                // On fielders choice, two events are fired; accept only the first.
                if event.metadata.sub_play == 0 {
                    let pitcher = pstat!();
                    if event.description.ends_with("double play!") {
                        pitcher.outs_recorded += 2;
                    } else {
                        pitcher.outs_recorded += 1;
                    }
                }
            }
            12 => {
                // Plate appearance
                ensure!(
                    event.player_tags.len() == 1,
                    "invalid player tags for type 12"
                );
                batter = Some((&event.player_tags[0]).into());
            }
            113 | 114 => {
                // Player swap
                let a_player = o!(&event.metadata.a_player_id);
                let b_player = o!(&event.metadata.b_player_id);
                for team in stats.iter_mut() {
                    for slot in team.slots_mut() {
                        if slot.last() == Some(a_player) {
                            slot.push(b_player.into());
                        } else if slot.last() == Some(b_player) {
                            slot.push(a_player.into());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(stats)
}
