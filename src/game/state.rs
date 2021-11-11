use crate::db::Db;
use crate::feed::GameEvent;
use crate::stats::{AwayHome, GameStats, Stats};
use crate::team;
use anyhow::{bail, ensure, Context, Result};
use indexmap::IndexMap;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub(crate) struct State<'a> {
    #[serde(skip)]
    db: &'a Db,

    pub(super) stats: AwayHome<GameStats>,
    game_started: bool,
    game_finished: bool,
    inning: u16,
    top_of_inning: bool,
    half_inning_outs: u16,
    at_bat: Option<Uuid>,
    last_fielded_out: Option<(u16, Uuid)>,
    rbi_credit: Option<Uuid>,
    // key: runner id, value: pitcher to be charged with the earned run
    on_base: IndexMap<Uuid, Uuid>,
}

impl<'a> State<'a> {
    pub(crate) fn new(db: &'a Db) -> State<'a> {
        State {
            db,
            stats: AwayHome::default(),
            game_started: false,
            game_finished: false,
            inning: 1,
            top_of_inning: true,
            half_inning_outs: 0,
            at_bat: None,
            last_fielded_out: None,
            rbi_credit: None,
            on_base: IndexMap::new(),
        }
    }

    pub(crate) fn finish(self) -> Result<AwayHome<GameStats>> {
        ensure!(self.game_finished, "game incomplete");
        let mut stats = self.stats;
        for team in stats.teams_mut() {
            team.totals = team.stats.values().copied().sum();
            ensure!(
                team.totals.outs_recorded % 3 == 0,
                "fractional total innings pitched"
            );
        }
        Ok(stats)
    }

    pub(crate) async fn push(&mut self, event: &GameEvent) -> Result<()> {
        self.push_inner(event)
            .await
            .with_context(|| format!("while processing event {}, type {}", event.id, event.ty))
    }

    async fn push_inner(&mut self, event: &GameEvent) -> Result<()> {
        if self.stats.away.pitchers.is_empty() {
            if let Some(pitchers) = &event.pitcher_data {
                self.stats.away.pitchers.push(pitchers.away_pitcher);
                self.stats
                    .away
                    .player_names
                    .insert(pitchers.away_pitcher, pitchers.away_pitcher_name.to_owned());
                self.stats.home.pitchers.push(pitchers.home_pitcher);
                self.stats
                    .home
                    .player_names
                    .insert(pitchers.home_pitcher, pitchers.home_pitcher_name.to_owned());
            }
        }

        let desc = &event.description;
        macro_rules! checkdesc {
            ($expr:expr) => {
                ensure!($expr, "unexpected event description: {:?}", desc)
            };
        }

        match event.ty {
            0 => self.start_event(event).await?,
            1 => {} // Play ball!
            2 => self.next_half_inning()?,
            3 => {
                // Pitcher change
                if let Some((name, _)) = desc.rsplit_once(" is now pitching for the ") {
                    ensure!(event.player_tags.len() == 1, "invalid player tag count");
                    self.defense_mut().pitchers.push(event.player_tags[0]);
                    self.defense_mut()
                        .player_names
                        .insert(event.player_tags[0], name.into());
                } else {
                    checkdesc!(false);
                }
            }
            4 => {
                // Stolen base
                checkdesc!(desc.contains("gets caught stealing") || desc.contains("steals"));
                ensure!(event.player_tags.len() == 1, "invalid player tag count");
                self.rbi_credit = None;
                if desc.contains("gets caught stealing") {
                    self.record_runner_event(event.player_tags[0], |s| &mut s.caught_stealing)?;
                    self.half_inning_outs += 1;
                    self.record_pitcher_event(|s| &mut s.outs_recorded)?;
                    self.on_base
                        .remove(&event.player_tags[0])
                        .context("runner caught stealing wasn't on base?")?;
                } else {
                    self.record_runner_event(event.player_tags[0], |s| &mut s.stolen_bases)?;
                    if desc.ends_with("steals fourth base!") {
                        self.credit_run(event.player_tags[0])?;
                    }
                }
            }
            5 => checkdesc!(self.walk(event)?),
            6 => {
                // Strikeout
                checkdesc!(desc.contains("strikes out"));
                self.record_batter_event(|s| &mut s.strike_outs)?;
                self.record_pitcher_event(|s| &mut s.struck_outs)?;
                self.batter_out(event)?;
            }
            7 | 8 => {
                // Flyout or ground out
                if desc.ends_with("reaches on fielder's choice.") {
                    // nothing, we already handled this in the "out at" branch
                } else {
                    checkdesc!(
                        desc.contains("hit a flyout to")
                            || desc.contains("hit a ground out to")
                            || desc.contains("out at")
                            || desc.ends_with("hit into a double play!")
                    );
                    self.fielded_out(event)?;
                }
            }
            9 => checkdesc!(self.home_run(event)?),
            10 => {
                if desc.ends_with("advances on the sacrifice.") {
                    self.sac(event)?;
                } else {
                    checkdesc!(self.hit(event)?);
                }
            }
            11 => {
                self.game_finished = true;
            }
            12 => {
                // Start of plate appearance
                ensure!(event.player_tags.len() == 1, "invalid player tag count");
                if let Some((name, _)) = desc.rsplit_once(" batting for the ") {
                    self.offense_mut()
                        .player_names
                        .insert(event.player_tags[0], name.into());
                    self.at_bat = Some(event.player_tags[0]);
                } else {
                    checkdesc!(false);
                }
            }
            13 => {
                // Strike
                checkdesc!(
                    desc.starts_with("Strike, looking.")
                        || desc.starts_with("Strike, swinging.")
                        || desc.starts_with("Strikes, looking.")
                        || desc.starts_with("Strikes, swinging.")
                );
                self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
            }
            14 => {
                // Ball
                checkdesc!(desc.starts_with("Ball."));
                self.record_pitcher_event(|s| &mut s.balls_pitched)?;
            }
            15 => {
                // Foul Ball
                checkdesc!(desc.starts_with("Foul Ball.") || desc.starts_with("Foul Balls."));
                self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
            }
            20 => {}  // Shame!
            23 => {}  // player skipped (Elsewhere or Shelled)
            28 => {}  // end of inning
            84 => {}  // player returned from Elsewhere
            107 => {} // removed modification
            132 => {
                checkdesc!(desc.ends_with("had their rotation shuffled in the Reverb!"));
                // do nothing, because type 3 will follow
            }
            209 => {} // score message
            214 => {} // team collected a Win
            216 => {} // game over
            223 => {} // weather is happening
            261 => {
                // Double strike
                checkdesc!(desc.ends_with("fires a Double Strike!"));
                // only record one extra strike; the next event catches the other
                self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
            }
            262 => {} // electricity zaps a strike away
            _ => bail!("unexpected event type {}", event.ty),
        }

        if usize::from(event.metadata.sub_play) == event.metadata.sibling_ids.len() - 1 {
            self.last_fielded_out = None;
            self.rbi_credit = None;

            if self.half_inning_outs < 3 {
                if let Some(base_runners) = &event.base_runners {
                    let mut known_runners = base_runners.clone();
                    known_runners.sort_unstable();
                    let mut derived_runners = self.on_base.keys().copied().collect::<Vec<_>>();
                    derived_runners.sort_unstable();
                    ensure!(
                        known_runners == derived_runners,
                        "baserunner mismatch: {:?} != {:?}",
                        known_runners,
                        derived_runners
                    );
                }
            }
        }

        Ok(())
    }

    async fn start_event(&mut self, event: &GameEvent) -> Result<()> {
        ensure!(event.team_tags.len() == 2, "invalid team tag count");
        self.stats.away.team = event.team_tags[0];
        self.stats.home.team = event.team_tags[1];

        for team in self.stats.teams_mut() {
            let data = team::load_team(self.db, team.team, event.created).await?;
            team.name = data.full_name;
            team.nickname = data.nickname;
            team.shorthand = data.shorthand;
            team.emoji = data.emoji;
            for player in data.lineup {
                team.lineup.push(vec![player]);
            }
        }

        Ok(())
    }

    fn next_half_inning(&mut self) -> Result<()> {
        self.offense_mut().left_on_base += self.on_base.len();

        if self.game_started {
            self.top_of_inning = !self.top_of_inning;
            if self.top_of_inning {
                self.inning += 1;
            }
        } else {
            self.game_started = true;
        }

        let inning = self.inning;
        self.offense_mut().inning_run_totals.insert(inning, 0);
        self.half_inning_outs = 0;
        self.on_base.clear();

        Ok(())
    }

    fn sac(&mut self, event: &GameEvent) -> Result<()> {
        ensure!(event.player_tags.len() == 1, "invalid player tag count");
        self.credit_run(event.player_tags[0])?;
        let (last_event, batter) = self
            .last_fielded_out
            .as_ref()
            .copied()
            .context("sac advance without a prior fielded out")?;
        let stats = self.offense_stats(batter);
        match last_event {
            7 => stats.sacrifice_flies += 1,
            8 => stats.sacrifice_hits += 1,
            _ => unreachable!(),
        }
        stats.runs_batted_in += 1;
        stats.at_bats -= 1;

        Ok(())
    }

    fn fielded_out(&mut self, event: &GameEvent) -> Result<()> {
        if let Some((out, _)) = event.description.rsplit_once(" out at ") {
            // fielder's choice
            self.record_pitcher_event(|s| &mut s.groundouts_pitched)?;
            let out = *self
                .offense()
                .player_names
                .iter()
                .find(|(_, name)| name == &out)
                .with_context(|| format!("could not determine id for baserunner {}", out))?
                .0;
            self.on_base
                .remove(&out)
                .context("baserunner out in fielder's choice not on base")?;
            self.on_base.insert(self.batter()?, self.pitcher()?);
        } else if event.description.ends_with("hit into a double play!") {
            // double play
            self.half_inning_outs += 1;
            self.rbi_credit = None;
            self.record_batter_event(|s| &mut s.double_plays_grounded_into)?;
            self.record_pitcher_event(|s| &mut s.groundouts_pitched)?;
            self.record_pitcher_event(|s| &mut s.outs_recorded)?;
            if self.on_base.len() == 1 {
                self.on_base.clear();
                self.offense_stats(self.batter()?).left_on_base += 1;
            } else if self.half_inning_outs == 2 {
                // this double play was made on one out, so it's the last play of the half-inning.
                // at this point it doesn't matter, so just add to player / team LOB correctly and
                // clear the baserunner list
                self.offense_stats(self.batter()?).left_on_base += self.on_base.len();
                self.offense_mut().left_on_base += self.on_base.len();
                self.on_base.pop();
            } else {
                // uh-oh. see the thing here is, the Feed doesn't tell us who the other
                // out was on, and we have multiple runners on. we'll need to rely on
                // the baseRunners object merged in from sachet.
                let base_runners = event
                    .base_runners
                    .as_ref()
                    .context("unable to determine runner out in double play")?;
                let out = self
                    .on_base
                    .keys()
                    // if more than one batter is removed, it's a scoring play; the other half of
                    // the double play will be on an earlier base and thus will have been added to
                    // this map _later_. reverse the iterator to find the latest one.
                    .rev()
                    .find(|runner| !base_runners.contains(runner))
                    .copied()
                    .context("unable to determine runner out in double play")?;
                self.on_base.remove(&out);
                self.offense_stats(self.batter()?).left_on_base += 1;
            }
        } else if event.description.contains("hit a flyout to") {
            self.record_pitcher_event(|s| &mut s.flyouts_pitched)?;
            self.last_fielded_out = self.at_bat.map(|id| (event.ty, id));
        } else if event.description.contains("hit a ground out to") {
            self.record_pitcher_event(|s| &mut s.groundouts_pitched)?;
            self.last_fielded_out = self.at_bat.map(|id| (event.ty, id));
        } else {
            unreachable!();
        }

        self.batter_out(event)
    }

    fn batter_out(&mut self, event: &GameEvent) -> Result<()> {
        self.half_inning_outs += 1;
        self.offense_stats(self.batter()?).left_on_base += self.on_base.len();
        self.record_batter_event(|s| &mut s.plate_appearances)?;
        self.record_batter_event(|s| &mut s.at_bats)?;
        if event.risp() {
            self.record_batter_event(|s| &mut s.at_bats_with_risp)?;
        }
        self.at_bat = None;
        self.record_pitcher_event(|s| &mut s.batters_faced)?;
        self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
        self.record_pitcher_event(|s| &mut s.outs_recorded)
    }

    fn credit_run(&mut self, runner: Uuid) -> Result<()> {
        let pitcher = self
            .on_base
            .remove(&runner)
            .context("cannot determine pitcher to charge with earned run")?;
        let inning = self.inning;
        *self
            .offense_mut()
            .inning_run_totals
            .entry(inning)
            .or_default() += 1;
        self.record_runner_event(runner, |s| &mut s.runs)?;
        if let Some(rbi_credit) = self.rbi_credit {
            self.record_runner_event(rbi_credit, |s| &mut s.runs_batted_in)?;
        }
        self.defense_mut()
            .stats
            .entry(pitcher)
            .or_default()
            .earned_runs += 1;
        Ok(())
    }

    fn walk(&mut self, event: &GameEvent) -> Result<bool> {
        if event.description.ends_with("draws a walk.") {
            self.on_base.insert(self.batter()?, self.pitcher()?);
            self.record_batter_event(|s| &mut s.plate_appearances)?;
            self.record_batter_event(|s| &mut s.walks)?;
            self.rbi_credit = self.at_bat;
            self.at_bat = None;
            self.record_pitcher_event(|s| &mut s.batters_faced)?;
            self.record_pitcher_event(|s| &mut s.walks_issued)?;
            Ok(true)
        } else if event.description.ends_with("scores!") {
            ensure!(event.player_tags.len() == 2, "invalid player tag count");
            self.credit_run(event.player_tags[1])?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn home_run(&mut self, event: &GameEvent) -> Result<bool> {
        if !(self.hit(event)?) {
            return Ok(false);
        }

        let was_on_base = self.on_base.clone();
        for (runner, _) in was_on_base {
            self.credit_run(runner)?;
        }
        self.on_base.clear();

        Ok(true)
    }

    fn hit(&mut self, event: &GameEvent) -> Result<bool> {
        macro_rules! common {
            () => {{
                self.on_base.insert(self.batter()?, self.pitcher()?);
                self.record_batter_event(|s| &mut s.plate_appearances)?;
                self.record_batter_event(|s| &mut s.at_bats)?;
                if event.risp() {
                    self.record_batter_event(|s| &mut s.at_bats_with_risp)?;
                    self.record_batter_event(|s| &mut s.hits_with_risp)?;
                }
                self.rbi_credit = self.at_bat;
                self.at_bat = None;
                self.record_pitcher_event(|s| &mut s.batters_faced)?;
                self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
                self.record_pitcher_event(|s| &mut s.hits_allowed)?;
                Ok(true)
            }};
        }

        let desc = &event.description;

        if event.ty == 9 && (desc.ends_with("home run!") || desc.ends_with("hits a grand slam!")) {
            self.record_batter_event(|s| &mut s.home_runs)?;
            self.record_pitcher_event(|s| &mut s.home_runs_allowed)?;
            common!()
        } else if event.ty == 10 && desc.ends_with("hits a Single!") {
            self.record_batter_event(|s| &mut s.singles)?;
            common!()
        } else if event.ty == 10 && desc.ends_with("hits a Double!") {
            self.record_batter_event(|s| &mut s.doubles)?;
            common!()
        } else if event.ty == 10 && desc.ends_with("hits a Triple!") {
            self.record_batter_event(|s| &mut s.triples)?;
            common!()
        } else if desc.ends_with("scores!") {
            ensure!(event.player_tags.len() == 1, "invalid player tag count");
            self.credit_run(event.player_tags[0])?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn offense(&self) -> &GameStats {
        if self.top_of_inning {
            &self.stats.away
        } else {
            &self.stats.home
        }
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
