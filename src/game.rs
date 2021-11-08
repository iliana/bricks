use crate::feed::GameEvent;
use crate::stats::{AwayHome, GameStats, Stats};
use crate::team;
use anyhow::{anyhow, ensure, Context, Result};
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct State {
    stats: AwayHome<GameStats>,
    game_started: bool,
    pub(crate) inning: u16,
    top_of_inning: bool,
    half_inning_outs: u8,
    at_bat: Option<Uuid>,
    on_base: u16,
}

impl State {
    pub(crate) fn new() -> State {
        State {
            stats: AwayHome::default(),
            game_started: false,
            inning: 0,
            top_of_inning: true,
            half_inning_outs: 0,
            at_bat: None,
            on_base: 0,
        }
    }

    pub(crate) async fn push(&mut self, event: &GameEvent) -> Result<()> {
        if self.stats.away.pitchers.is_empty() {
            if let Some(away_pitcher) = event.away_pitcher {
                if let Some(home_pitcher) = event.home_pitcher {
                    self.stats.away.pitchers.push(away_pitcher);
                    self.stats.home.pitchers.push(home_pitcher);
                }
            }
        }

        let desc = &event.description;
        macro_rules! checkdesc {
            ($expr:expr) => {
                ensure!($expr, "unexpected event description: {:?}", desc);
            };
        }

        match event.ty {
            0 => self.start_event(event).await,
            1 => Ok(()), // Play ball!
            2 => self.next_half_inning(),
            3 => {
                // Pitcher change
                ensure!(event.player_tags.len() == 1, "invalid player tag count");
                self.defense_mut().pitchers.push(event.player_tags[0]);
                Ok(())
            }
            4 => {
                // Stolen base
                ensure!(event.player_tags.len() == 1, "invalid player tag count");
                self.record_runner_event(event.player_tags[0], |s| &mut s.stolen_bases)
            }
            7 | 8 => self.flyout_groundout(event),
            10 => {
                // Non-HR hit
                checkdesc!(
                    desc.ends_with("hits a Single!")
                        || desc.ends_with("hits a Double!")
                        || desc.ends_with("hits a Triple!")
                );
                self.hit(event)
            }
            12 => {
                // Start of plate appearance
                ensure!(event.player_tags.len() == 1, "invalid player tag count");
                self.at_bat = Some(event.player_tags[0]);
                Ok(())
            }
            13 => {
                // Strike
                checkdesc!(
                    desc.starts_with("Strike, looking.") || desc.starts_with("Strike, swinging.")
                );
                self.record_pitcher_event(|s| &mut s.strikes_pitched)
            }
            14 => {
                // Ball
                checkdesc!(desc.starts_with("Ball."));
                self.record_pitcher_event(|s| &mut s.balls_pitched)
            }
            15 => {
                // Foul Ball
                checkdesc!(desc.starts_with("Foul Ball."));
                self.record_pitcher_event(|s| &mut s.strikes_pitched)
            }
            28 => Ok(()), // end of inning
            132 => {
                checkdesc!(desc.ends_with("had their rotation shuffled in the Reverb!"));
                // do nothing, because type 3 will follow
                Ok(())
            }
            223 => Ok(()), // weather is happening
            _ => Err(anyhow!("unexpected event type")),
        }
        .with_context(|| format!("while processing event {}, type {}", event.id, event.ty))?;

        if let Some(base_runners) = &event.base_runners {
            if self.half_inning_outs < 3 {
                ensure!(
                    usize::from(self.on_base) == base_runners.len(),
                    "baserunner count mismatch, {} != {}, event {}",
                    self.on_base,
                    base_runners.len(),
                    event.id
                );
            }
        }

        Ok(())
    }

    async fn start_event(&mut self, event: &GameEvent) -> Result<()> {
        ensure!(event.team_tags.len() == 2, "invalid team tag count");
        self.stats.away.team = event.team_tags[0];
        self.stats.home.team = event.team_tags[1];

        for team in self.stats.iter_mut() {
            let data = team::load_team(&team.team.to_string(), event.created).await?;
            for player in data.lineup {
                team.lineup.push(vec![player]);
            }
        }

        Ok(())
    }

    fn next_half_inning(&mut self) -> Result<()> {
        if self.game_started {
            self.top_of_inning = !self.top_of_inning;
            if self.top_of_inning {
                self.inning += 1;
            }
        } else {
            self.game_started = true;
        }

        self.half_inning_outs = 0;
        self.on_base = 0;

        Ok(())
    }

    fn flyout_groundout(&mut self, event: &GameEvent) -> Result<()> {
        let desc = &event.description;
        macro_rules! checkdesc {
            ($expr:expr) => {
                ensure!($expr, "unexpected event description: {:?}", desc);
            };
        }

        if event.metadata.sub_play == 0 {
            checkdesc!(
                desc.contains("hit a flyout to")
                    || desc.contains("hit a ground out to")
                    || desc.contains("out at")
            );
            if desc.contains("out at") {
                ensure!(
                    event.metadata.sibling_ids.len() == 2,
                    "incorrect number of events for fielder's choice"
                );
            }
            self.half_inning_outs += 1;
            self.record_batter_event(|s| &mut s.plate_appearances)?;
            self.record_batter_event(|s| &mut s.at_bats)?;
            self.at_bat = None;
            self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
            self.record_pitcher_event(|s| &mut s.outs_recorded)
        } else if event.metadata.sub_play == 1 {
            checkdesc!(desc.ends_with("reaches on fielder's choice."));
            Ok(())
        } else {
            checkdesc!(false);
            Ok(())
        }
    }

    fn hit(&mut self, event: &GameEvent) -> Result<()> {
        let desc = &event.description;

        if desc.ends_with("Single!") {
            self.record_batter_event(|s| &mut s.singles)?;
        } else if desc.ends_with("Double!") {
            self.record_batter_event(|s| &mut s.doubles)?;
        } else if desc.ends_with("Triple!") {
            self.record_batter_event(|s| &mut s.triples)?;
        }
        self.on_base += 1;
        self.record_batter_event(|s| &mut s.plate_appearances)?;
        self.record_batter_event(|s| &mut s.at_bats)?;
        self.at_bat = None;
        self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
        self.record_pitcher_event(|s| &mut s.hits_allowed)
    }

    fn offense_mut(&mut self) -> &mut GameStats {
        if self.top_of_inning {
            &mut self.stats.away
        } else {
            &mut self.stats.home
        }
    }

    fn defense(&self) -> &GameStats {
        if self.top_of_inning {
            &self.stats.home
        } else {
            &self.stats.away
        }
    }

    fn defense_mut(&mut self) -> &mut GameStats {
        if self.top_of_inning {
            &mut self.stats.home
        } else {
            &mut self.stats.away
        }
    }

    fn offense_stats(&mut self, player: Uuid) -> &mut Stats {
        self.offense_mut().stats.entry(player).or_default()
    }

    fn defense_stats(&mut self, player: Uuid) -> &mut Stats {
        self.defense_mut().stats.entry(player).or_default()
    }

    fn batter(&self) -> Result<Uuid> {
        self.at_bat.context("nobody at bat")
    }

    fn pitcher(&self) -> Result<Uuid> {
        self.defense()
            .pitchers
            .last()
            .copied()
            .context("unknown pitcher")
    }

    fn record_batter_event<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Stats) -> &mut u16,
    {
        let batter = self.batter()?;
        *f(self.offense_stats(batter)) += 1;
        Ok(())
    }

    fn record_runner_event<F>(&mut self, runner: Uuid, f: F) -> Result<()>
    where
        F: FnOnce(&mut Stats) -> &mut u16,
    {
        *f(self.offense_stats(runner)) += 1;
        Ok(())
    }

    fn record_pitcher_event<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Stats) -> &mut u16,
    {
        let pitcher = self.pitcher()?;
        *f(self.defense_stats(pitcher)) += 1;
        Ok(())
    }
}
