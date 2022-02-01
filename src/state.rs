use crate::feed::{ExtraData, GameEvent};
use crate::game::{Game, Kind, Stats, Team};
use crate::{seasons::Season, team};
use anyhow::{anyhow, bail, ensure, Context, Result};
use chrono::Duration;
use itertools::Itertools;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use uuid::Uuid;

type PitcherData = (u128, [(u128, &'static str); 2]);

const HARDCODED_PITCHERS: &[PitcherData] = &[
    // At initial processing time, 3323cff9-881c-4114-bcdc-87622bc9f218 was missing data in
    // Chronicler. This hardcoded data is sourced from the backup archiver.
    (
        0x3323cff9881c4114bcdc87622bc9f218,
        [
            (0xe878bdc355254808912a5dbab25c24e7, "Hazel Smithson"),
            (0xf5c7b32971aa4241a8dfbf34d106f757, "Yahya Jupiter"),
        ],
    ),
];

#[cfg(test)]
#[test]
fn test_hardcoded_pitchers_sorted() {
    let mut v = HARDCODED_PITCHERS.to_vec();
    v.sort();
    assert_eq!(v, HARDCODED_PITCHERS);
}

#[derive(Debug, Serialize)]
pub struct State {
    id: Uuid,
    game: Game,
    game_started: bool,
    game_finished: bool,
    inning: u16,
    top_of_inning: bool,
    #[serde(skip)]
    last_runs_cmp: Ordering,
    half_inning_outs: u16,
    at_bat: Option<Uuid>,
    last_fielded_out: Option<Uuid>,
    rbi_credit: Option<Uuid>,
    save_situation: [Option<SaveSituation>; 2],
    on_base: Vec<Runner>,
    #[serde(skip)]
    on_base_start_of_play: Vec<Runner>,
    #[serde(skip)]
    expected: (u16, u16),
    #[serde(skip)]
    mods: HashSet<(Uuid, &'static str)>,
}

impl State {
    pub fn new(season: Season, id: Uuid) -> State {
        let mut game = Game {
            season,
            ..Default::default()
        };

        let data = match HARDCODED_PITCHERS.binary_search_by_key(&id.as_u128(), |(id, _)| *id) {
            Ok(idx) => HARDCODED_PITCHERS[idx],
            Err(_) => Default::default(),
        };
        for (team, pitcher) in game.teams_mut().zip(data.1) {
            let id = Uuid::from_u128(pitcher.0);
            team.pitchers.push(id);
            team.player_names.insert(id, pitcher.1.into());
            team.stats.insert(
                id,
                Stats {
                    games_started: 1,
                    ..Stats::default()
                },
            );
        }

        State {
            id,
            game,
            game_started: false,
            game_finished: false,
            inning: 1,
            top_of_inning: true,
            last_runs_cmp: Ordering::Equal,
            half_inning_outs: 0,
            at_bat: None,
            last_fielded_out: None,
            rbi_credit: None,
            save_situation: [None; 2],
            on_base: Vec::new(),
            on_base_start_of_play: Vec::new(),
            expected: (0, 0),
            mods: HashSet::new(),
        }
    }

    pub fn finish(self) -> Result<Game> {
        ensure!(self.game_finished, "game incomplete");
        self.ensure_pitchers_known()?;
        let mut game = self.game;
        ensure!(game.away.won ^ game.home.won, "winner mismatch");

        for (i, team) in game.teams_mut().enumerate() {
            // remove any players with all-zero stats and clean up references to them
            team.stats.retain(|_, stats| stats != &Stats::default());
            team.player_names
                .retain(|id, _| team.stats.contains_key(id));
            for position in &mut team.lineup {
                position.retain(|id| team.stats.contains_key(id));
            }
            team.pitchers.retain(|id| team.stats.contains_key(id));
            team.lineup.retain(|position| !position.is_empty());

            ensure!(
                team.pitchers.iter().all(|id| id != &Uuid::default()),
                "placeholder pitcher ID found"
            );
            for position in team.positions() {
                for player in position {
                    ensure!(
                        team.player_names.contains_key(player),
                        "name for {} missing",
                        player
                    );
                }
            }

            if team.pitcher_of_record == Uuid::default() {
                // the starting pitcher was cleared as the pitcher of record because they pitched
                // less than 5 innings, making them ineligible for the win ...
                if team.won {
                    // ... but no one else became the pitcher of record. choose the relief pitcher
                    // who pitched the longest i guess.
                    team.pitcher_of_record = team
                        .pitchers
                        .iter()
                        .copied()
                        .skip(1)
                        .rev()
                        .max_by_key(|pitcher| {
                            team.stats
                                .get(pitcher)
                                .copied()
                                .unwrap_or_default()
                                .outs_recorded
                        })
                        .unwrap_or_default();
                } else {
                    // ... but because their team lost and no other pitcher became the new pitcher
                    // of record, they're the losing pitcher.
                    team.pitcher_of_record = team.pitchers.first().copied().unwrap_or_default();
                }
            }
            ensure!(
                team.pitcher_of_record != Uuid::default(),
                "placeholder pitcher ID listed as winning or losing pitcher"
            );
            if team.won {
                team.stats.entry(team.pitcher_of_record).or_default().wins = 1;
            } else {
                team.stats.entry(team.pitcher_of_record).or_default().losses = 1;
            }

            if team.won {
                let finishing_pitcher = *team.pitchers.last().unwrap();
                if team.pitcher_of_record != finishing_pitcher {
                    let stats = team.stats.entry(finishing_pitcher).or_default();
                    let save = match self.save_situation[i] {
                        Some(SaveSituation::TyingRun) => stats.outs_recorded >= 1,
                        Some(SaveSituation::LeadThreeOrLess) => stats.outs_recorded >= 3,
                        None => stats.outs_recorded >= 9,
                    };
                    if save {
                        team.saving_pitcher = Some(finishing_pitcher);
                        stats.saves = 1;
                    }
                }
            }

            ensure!(
                team.stats
                    .values()
                    .map(|stats| stats.outs_recorded)
                    .sum::<u32>()
                    % 3
                    == 0,
                "fractional total innings pitched"
            );
            ensure!(
                !team.stats.contains_key(&Uuid::default()),
                "placeholder pitcher ID present in stats"
            );
            ensure!(
                !team.player_names.contains_key(&Uuid::default()),
                "placeholder pitcher ID present in player names"
            );
            for stats in team.stats.values_mut() {
                if stats.is_batting() {
                    stats.games_batted += 1;
                }
                if stats.is_pitching() {
                    stats.games_pitched += 1;
                }
            }
        }

        Ok(game)
    }

    fn ensure_pitchers_known(&self) -> Result<()> {
        ensure!(
            self.game
                .teams()
                .all(|team| *team.pitchers.first().unwrap() != Uuid::default()),
            "initial pitchers are unknown"
        );
        Ok(())
    }

    fn name_lookup(&self, name: &str, id: Option<Uuid>) -> Result<Uuid> {
        id.or_else(|| {
            self.offense()
                .player_names
                .iter()
                .find(|(_, n)| n.as_str() == name)
                .map(|(id, _)| *id)
        })
        .ok_or_else(|| anyhow!("unable to find ID for {}", name))
    }

    pub async fn push(&mut self, event: &GameEvent) -> Result<()> {
        self.push_inner(event)
            .await
            .with_context(|| format!("while processing event {}, type {}", event.id, event.ty))
    }

    async fn push_inner(&mut self, event: &GameEvent) -> Result<()> {
        if event.id.as_u128() == 0x2ca7226183224b86af4e570aa0dd1deb {
            // something bizarre happened in gmae f52eedb9-da6e-45db-8147-3b64fb260dbb -- the sim
            // paused for about 39 seconds after a type 12 ("batting for the") event, then repeated
            // the same event with the same play and subplay numbers. skip the first one.
            return Ok(());
        }

        self.expected = event.expect(self.expected)?;

        if self.is_game_over() {
            if event.ty == 107 && event.metadata.r#mod.as_deref() == Some("INHABITING") {
                // sometimes this happens!
            } else {
                ensure!(self.game_finished || event.ty == 11, "game over mismatch");
            }
        }

        for (team, is_defense, pitcher, pitcher_name) in [
            (
                &mut self.game.away,
                !self.top_of_inning,
                &event.away_pitcher,
                &event.away_pitcher_name,
            ),
            (
                &mut self.game.home,
                self.top_of_inning,
                &event.home_pitcher,
                &event.home_pitcher_name,
            ),
        ] {
            if let (Some(pitcher), Some(pitcher_name)) = (pitcher, pitcher_name) {
                if team.replace_placeholder_pitcher(*pitcher, pitcher_name) && is_defense {
                    for runner in &mut self.on_base {
                        if runner.pitcher == Uuid::default() {
                            runner.pitcher = *pitcher;
                        }
                    }
                }
                team.player_names
                    .entry(*pitcher)
                    .or_insert_with(|| pitcher_name.to_string());
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

                    if let Err(err) = self.ensure_pitchers_known() {
                        // Starting in gamma9, the sim does not correctly set the pitcher before
                        // the start of a tournament's first game, leading to a pitcher change
                        // event at the start of each half of the first inning because the sim is
                        // changing the pitcher from "null" to whoever the starting pitcher is. If
                        // it's the first inning and the starting pitcher is a placeholder who has
                        // not yet thrown the ball, we should ignore this error.
                        ensure!(
                            self.inning == 1
                                && self.defense().pitchers.len() == 1
                                && self.defense_stats(self.pitcher())
                                    == &Stats {
                                        games_started: 1,
                                        ..Default::default()
                                    },
                            err
                        );

                        // If we still have placeholder pitcher data for the defense, fill in the
                        // data we now have so that the pitcher change branch below doesn't run.
                        self.defense_mut()
                            .replace_placeholder_pitcher(event.player_tags[0], name);
                    }

                    if self.pitcher() != event.player_tags[0] {
                        // starting pitchers must pitch 5 innings to be credited for the win. clear
                        // the pitcher of record if they are not eligible to record the win; if the
                        // losing team's pitcher is still cleared by the end of the game, fill it
                        // back in with the starting pitcher for the losing team.
                        let old_pitcher = self.pitcher();
                        if self.defense().pitchers.len() == 1
                            && self.defense_stats(old_pitcher).outs_recorded < 15
                        {
                            self.defense_mut().pitcher_of_record = Uuid::default();
                        }

                        self.defense_mut().pitchers.push(event.player_tags[0]);
                        self.defense_mut()
                            .player_names
                            .insert(event.player_tags[0], name.into());

                        let offense_runs = self.offense().runs();
                        let defense_runs = self.defense().runs();
                        let save = &mut self.save_situation[if self.top_of_inning { 1 } else { 0 }];
                        *save = if defense_runs > offense_runs {
                            if offense_runs + 1 >= defense_runs
                                || offense_runs + u16::try_from(self.on_base.len())? >= defense_runs
                            {
                                // potential tying run on base or at bat
                                // NOTE: on deck handled via `check_save_situation`
                                Some(SaveSituation::TyingRun)
                            } else if defense_runs - offense_runs <= 3 {
                                Some(SaveSituation::LeadThreeOrLess)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                    }
                } else {
                    checkdesc!(false);
                }
            }
            4 => {
                // Stolen base
                if let Some((name, _)) = desc.rsplit_once(" gets caught stealing ") {
                    checkdesc!(desc.ends_with(" base."));
                    let runner = self.name_lookup(name, event.player_tags.get(0).copied())?;
                    self.record_runner_event(runner, |s| &mut s.caught_stealing)?;
                    self.half_inning_outs += 1;
                    self.record_pitcher_event(|s| &mut s.outs_recorded)?;
                    self.remove_runner(runner)?
                        .context("runner caught stealing wasn't on base?")?;
                } else if let Some((name, _)) = desc.rsplit_once(" steals ") {
                    checkdesc!(desc.ends_with(" base!"));
                    let runner = self.name_lookup(name, event.player_tags.get(0).copied())?;
                    self.record_runner_event(runner, |s| &mut s.stolen_bases)?;
                    if desc.ends_with("steals fourth base!") {
                        self.credit_run(runner)?;
                    }
                } else {
                    checkdesc!(false);
                }
            }
            5 => checkdesc!(self.walk(event)?),
            6 => {
                // Strikeout
                checkdesc!(desc.contains("strikes out"));
                self.record_batter_event(|s| &mut s.strike_outs)?;
                self.record_pitcher_event(|s| &mut s.struck_outs)?;
                self.batter_out()?;
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
                if desc.ends_with("advances on the sacrifice.")
                    || desc.ends_with("tags up and scores!")
                {
                    self.sac(event)?;
                } else {
                    checkdesc!(self.hit(event)?);
                }
            }
            11 => {
                self.game_finished = true;
                let winner = event.metadata.winner.context("missing winner data")?;
                for team in self.game.teams_mut() {
                    if team.id == winner {
                        team.won = true;
                    }
                }
                for team in self.game.teams_mut() {
                    let stats = team
                        .stats
                        .entry(*team.pitchers.last().unwrap())
                        .or_default();
                    stats.games_finished = 1;
                    if stats.games_started > 0 {
                        stats.complete_games = 1;
                        if stats.earned_runs == 0 {
                            stats.shutouts = 1;
                            if stats.hits_allowed == 0 {
                                stats.no_hitters = 1;
                                if stats.walks_issued == 0 {
                                    stats.perfect_games = 1;
                                }
                            }
                        }
                    }
                }
            }
            12 => {
                // Start of plate appearance
                if event.player_tags.len() == 2
                    && desc.contains("is Inhabiting")
                    && !desc.contains("batting for the")
                {
                    let position = self
                        .offense_mut()
                        .positions_mut()
                        .find(|position| position.last() == Some(&event.player_tags[1]))
                        .context("unable to find position for inhabited player")?;
                    position.push(event.player_tags[0]);
                    position.push(event.player_tags[1]);
                } else {
                    ensure!(
                        event.player_tags.len() == 1 || event.player_tags.len() == 2,
                        "invalid player tag count"
                    );
                    if let Some((name, _)) = desc.rsplit_once(" batting for the ") {
                        self.offense_mut()
                            .player_names
                            .insert(event.player_tags[0], name.into());
                        self.at_bat = Some(event.player_tags[0]);
                    } else {
                        checkdesc!(false);
                    }
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
            20 => {} // Shame!
            23 => {} // player skipped (Elsewhere or Shelled)
            24 => {} // partying
            28 => {} // end of inning
            41 => {
                if desc.ends_with("switch teams in the feedback!") {
                    ensure!(event.player_tags.len() == 2, "invalid team tag count");
                    self.player_trade(event.player_tags[0], event.player_tags[1])?;
                }
            }
            46 => {} // yummy peanut reaction
            47 => {} // allergic peanut reaction
            54 => {} // incineration
            62 => {
                // Flooding: baserunners swept
                checkdesc!(desc == "A surge of Immateria rushes up from Under!\nBaserunners are swept from play!");
                self.on_base.clear();
            }
            73 => {} // peanut flavor text
            84 => {} // player returned from Elsewhere
            106 | 107 | 146 | 147 => {
                // modification added or removed
                match event.metadata.r#mod.as_deref() {
                    Some("FROZEN") => {
                        if let Some(name) = desc.strip_suffix(" was Frozen!") {
                            // we only care about FROZEN for calculating CRiSP, which requires
                            // that they're on base. if we can't look up their name, they can't
                            // be on base.
                            if let Ok(player) =
                                self.name_lookup(name, event.player_tags.get(0).copied())
                            {
                                if event.ty & 1 == 0 {
                                    self.mods.insert((player, "FROZEN"));
                                } else {
                                    self.mods.remove(&(player, "FROZEN"));
                                }
                            }
                        }
                    }
                    Some(_) => {}
                    None => bail!("missing modification data"),
                }
            }
            113 => {
                // Trade (e.g. Feedback swap)
                // for feedback, handled in 41
                checkdesc!(desc.ends_with("were swapped in Feedback."));
            }
            114 => {
                // Swap within team
                checkdesc!(
                    desc.ends_with("swapped two players on their roster.")
                        || desc.ends_with("had several players shuffled in the Reverb!")
                );
                let swap = match &event.metadata.extra {
                    Some(ExtraData::Swap(swap)) => swap,
                    _ => bail!("missing player swap data"),
                };
                self.player_trade(swap.a_player_id, swap.b_player_id)?;
                // yolo
                for (player, player_name) in [
                    (swap.a_player_id, &swap.a_player_name),
                    (swap.b_player_id, &swap.b_player_name),
                ] {
                    for team in self.game.teams_mut() {
                        team.player_names.insert(player, player_name.clone());
                    }
                }
            }
            116 => {
                // Incineration
                if desc.contains("replaced the incinerated") {
                    self.ensure_pitchers_known()?;
                    let replacement = match &event.metadata.extra {
                        Some(ExtraData::Incineration(replacement)) => replacement,
                        _ => bail!("missing incineration replacement data"),
                    };
                    for team in self.game.teams_mut() {
                        if team.id == replacement.team_id {
                            team.player_names.insert(
                                replacement.in_player_id,
                                replacement.in_player_name.clone(),
                            );
                            for position in team.positions_mut() {
                                if position.last() == Some(&replacement.out_player_id) {
                                    position.push(replacement.in_player_id);
                                }
                            }
                        }
                    }
                } else if desc.starts_with("They're replaced by") {
                    // nothing, redundant event
                } else {
                    checkdesc!(false);
                }
            }
            117 => {} // player stat increase
            118 => {} // player stat decrease
            119 => {} // player stat reroll
            125 => {} // player entered Hall of Flame
            130 | 131 => {
                // Reverb. 130 is a full-team shuffle, and 131 is a lineup shuffle. Players are not
                // swapped, but instead the order is shuffled, so there's no information in the
                // feed about the new lineup order.
                //
                // To get the new roster order, we ask Chronicler for the team information one
                // minute after this feed event, since it fetches team data once per minute during
                // games.
                //
                // This only sets the new lineup order. Pitching changes (if any) are handled via
                // type 3.

                // The feed doesn't specify which team got Reverbed, so we need to scan the event
                // description for the team's nickname.
                let team = self
                    .game
                    .teams_mut()
                    .find(|team| desc.contains(&team.name.nickname))
                    .context("could not identify reverbed team")?;
                let data = team::load(team.id, event.created + Duration::minutes(1))
                    .await?
                    .context("no data for team")?;
                ensure!(
                    team.lineup.len() == data.lineup.len(),
                    "lineup size mismatch"
                );
                for (position, player) in team.lineup.iter_mut().zip(data.lineup) {
                    if position.last() != Some(&player) {
                        position.push(player);
                    }
                }
            }
            132 => {
                checkdesc!(desc.ends_with("had their rotation shuffled in the Reverb!"));
                // do nothing, because type 3 will follow
            }
            137 => {} // player hatched
            193 => {
                // prize match
                self.game.kind = Kind::Special;
            }
            209 => {} // score message
            214 => {} // collected a Win
            215 => {} // collected a Win (postseason, sometimes)
            216 => {} // game over
            223 => {} // weather is happening
            252 => {} // Night Shift (handled in type 114)
            261 => {
                // Double strike
                checkdesc!(desc.ends_with("fires a Double Strike!"));
                // only record one extra strike; the next event catches the other
                self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
            }
            262 => {} // electricity zaps a strike away
            263 => {} // WINTER STORM WARNING
            264 => {} // snowflakes modify the field
            265 => {} // player is Unfreezable
            _ => bail!("unexpected event type {}", event.ty),
        }

        if usize::from(event.metadata.sub_play) == event.metadata.sibling_ids.len() - 1 {
            self.last_fielded_out = None;
            self.rbi_credit = None;

            for (team, pitcher) in [
                (&mut self.game.away, &event.away_pitcher),
                (&mut self.game.home, &event.home_pitcher),
            ] {
                if let Some(pitcher) = pitcher {
                    ensure!(team.pitchers.last().unwrap() == pitcher, "pitcher mismatch");
                }
            }

            if self.half_inning_outs < 3 {
                for runner in &self.on_base {
                    ensure!(
                        runner.base < 3,
                        "baserunner {} should have scored",
                        runner.id
                    );
                }

                if let (Some(base_runners), Some(bases_occupied)) =
                    (&event.base_runners, &event.bases_occupied)
                {
                    let mut known = bases_occupied
                        .iter()
                        .copied()
                        .zip(base_runners.iter().copied())
                        .collect::<Vec<_>>();
                    known.sort_unstable();
                    known.reverse();
                    for runner in &mut self.on_base {
                        let index = known
                            .iter()
                            .position(|(_, id)| runner.id == *id)
                            .context("baserunner {} missing from event")?;
                        let (base, _) = known.remove(index);
                        ensure!(
                            base >= runner.base,
                            "baserunner {} on base {} but should be on at least {}",
                            runner.id,
                            base,
                            runner.base,
                        );
                        runner.base = base;
                    }
                    ensure!(known.is_empty(), "baserunners {:?} not known to us", known);
                }
            }

            let crisp = self
                .on_base
                .iter()
                .filter_map(|runner| {
                    (runner.base > 0 && self.mods.contains(&(runner.id, "FROZEN")))
                        .then(|| runner.id)
                })
                .collect::<Vec<_>>();
            self.offense_mut().crisp.extend(crisp);

            self.last_runs_cmp = self.runs_cmp();
            self.on_base_start_of_play = self.on_base.clone();
        }

        Ok(())
    }

    fn is_game_over(&self) -> bool {
        self.inning >= 9
            && !self.top_of_inning
            && self.half_inning_outs == 3
            && self.game.away.runs() != self.game.home.runs()
    }

    fn check_save_situation(&mut self) {
        if self.defense_stats(self.pitcher()).batters_faced == 1
            && self.defense().runs() > self.offense().runs()
            && self.offense().runs() + 1 >= self.defense().runs()
            && !self.is_game_over()
        {
            // potential tying run on deck as of pitcher's start
            self.save_situation[if self.top_of_inning { 1 } else { 0 }] =
                Some(SaveSituation::TyingRun);
        }
    }

    async fn start_event(&mut self, event: &GameEvent) -> Result<()> {
        self.game.day = event.day;
        self.game.weather = event.metadata.weather.context("missing weather")?;

        self.game.kind = if self.game.season.sim == "gamma8" {
            if self.game.day >= 99 {
                Kind::Postseason
            } else {
                Kind::Regular
            }
        } else if self.game.season.sim == "gamma9" {
            if self.game.day >= 166 {
                Kind::Postseason
            } else {
                Kind::Regular
            }
        } else if self.game.season.sim == "gamma10" {
            if self.game.day >= 219 {
                // probably gonna have to fix this
                Kind::Postseason
            } else {
                Kind::Regular
            }
        } else {
            Kind::Regular
        };

        ensure!(event.team_tags.len() == 2, "invalid team tag count");
        for (team, id) in self.game.teams_mut().zip(event.team_tags.iter()) {
            team.id = *id;
        }

        for team in self.game.teams_mut() {
            let data = team::load(team.id, event.created)
                .await?
                .context("no data for team")?;
            team.name.name = data.full_name;
            team.name.nickname = data.nickname;
            team.name.shorthand = data.shorthand;
            team.name.emoji = data.emoji;
            for player in data.lineup {
                team.lineup.push(vec![player]);
            }
        }

        Ok(())
    }

    fn risp(&self) -> bool {
        self.on_base_start_of_play
            .iter()
            .any(|runner| runner.base >= 1)
    }

    fn remove_runner(&mut self, id: Uuid) -> Result<Option<Runner>> {
        match self
            .on_base
            .iter()
            .enumerate()
            .filter(|(_, runner)| runner.id == id)
            .at_most_one()
        {
            Ok(Some((index, _))) => Ok(Some(self.on_base.remove(index))),
            Ok(None) => Ok(None),
            Err(_) => {
                bail!("can't determine which {} to remove from bases", id)
            }
        }
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
        self.offense_mut().inning_runs.insert(inning, 0);
        self.half_inning_outs = 0;
        self.on_base.clear();

        Ok(())
    }

    fn sac(&mut self, event: &GameEvent) -> Result<()> {
        ensure!(event.player_tags.len() == 1, "invalid player tag count");
        self.credit_run(event.player_tags[0])?;
        let batter = self
            .last_fielded_out
            .as_ref()
            .copied()
            .context("sac advance without a prior fielded out")?;
        let risp = self.risp();
        let stats = self.offense_stats(batter);
        stats.sacrifices += 1;
        stats.runs_batted_in += 1;
        stats.at_bats -= 1;
        if risp {
            stats.at_bats_with_risp -= 1;
        }

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
            let pitcher = self
                .remove_runner(out)?
                .context("baserunner out in fielder's choice not on base")?
                .pitcher;
            self.on_base.push(Runner {
                id: self.batter()?,
                pitcher,
                base: 0,
            });
            self.fix_minimum_base();
        } else if event.description.ends_with("hit into a double play!") {
            // double play
            self.half_inning_outs += 1;
            self.rbi_credit = None;
            self.record_batter_event(|s| &mut s.double_plays_grounded_into)?;
            self.record_pitcher_event(|s| &mut s.groundouts_pitched)?;
            self.record_pitcher_event(|s| &mut s.outs_recorded)?;
            if event.id.as_u128() == 0x3fdb026f97a3401385ee44f935c26f01 {
                // missing data in Chronicler at the start of 5ffbde13-1807-4694-9d13-861c6302b384.
                // the runner put out was Craig Faucet.
                self.remove_runner(Uuid::from_u128(0xe34b37e1b47448ed8a657e182733996c))?;
                self.offense_stats(self.batter()?).left_on_base += 1;
            } else if self.on_base.len() == 1 {
                self.on_base.clear();
                self.offense_stats(self.batter()?).left_on_base += 1;
            } else if self.half_inning_outs == 2 {
                // this double play was made on one out, so it's the last play of the half-inning.
                // at this point it doesn't matter, so just add to player / team LOB correctly and
                // clear the baserunner list
                self.offense_stats(self.batter()?).left_on_base += self.on_base.len();
                self.offense_mut().left_on_base += self.on_base.len();
                self.on_base.clear();
            } else {
                // uh-oh. we have multiple runners on, but the Feed doesn't tell us which one is
                // out. we'll need to rely on the baseRunners object merged in from sachet.
                let base_runners = event
                    .base_runners
                    .as_ref()
                    .context("unable to determine runner out in double play")?;
                let out = self
                    .on_base
                    .iter()
                    // if more than one batter is removed, it's a scoring play; the other half of
                    // the double play will be on an earlier base and thus will have been added to
                    // this map _later_. reverse the iterator to find the latest one.
                    .rev()
                    .find(|runner| !base_runners.contains(&runner.id))
                    .map(|runner| runner.id)
                    .context("unable to determine runner out in double play")?;
                self.remove_runner(out)?;
                self.offense_stats(self.batter()?).left_on_base += 1;
            }
        } else if event.description.contains("hit a flyout to") {
            self.record_pitcher_event(|s| &mut s.flyouts_pitched)?;
            self.last_fielded_out = self.at_bat;
        } else if event.description.contains("hit a ground out to") {
            self.record_pitcher_event(|s| &mut s.groundouts_pitched)?;
            self.last_fielded_out = self.at_bat;
        } else {
            unreachable!();
        }

        self.batter_out()
    }

    fn batter_out(&mut self) -> Result<()> {
        self.half_inning_outs += 1;
        self.offense_stats(self.batter()?).left_on_base += self.on_base.len();
        self.record_batter_event(|s| &mut s.plate_appearances)?;
        self.record_batter_event(|s| &mut s.at_bats)?;
        if self.risp() {
            self.record_batter_event(|s| &mut s.at_bats_with_risp)?;
        }
        self.at_bat = None;
        self.record_pitcher_event(|s| &mut s.batters_faced)?;
        self.check_save_situation();
        self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
        self.record_pitcher_event(|s| &mut s.outs_recorded)
    }

    fn credit_run(&mut self, runner: Uuid) -> Result<()> {
        // instead of calling `remove_runner` here, we can assume the first runner in the list is
        // the one that scored
        let index = self
            .on_base
            .iter()
            .position(|r| r.id == runner)
            .context("cannot determine pitcher to charge with earned run")?;
        let pitcher = self.on_base.remove(index).pitcher;

        let inning = self.inning;
        *self.offense_mut().inning_runs.entry(inning).or_default() += 1;
        self.record_runner_event(runner, |s| &mut s.runs)?;
        if let Some(rbi_credit) = self.rbi_credit {
            self.record_runner_event(rbi_credit, |s| &mut s.runs_batted_in)?;
        }
        self.defense_mut()
            .stats
            .entry(pitcher)
            .or_default()
            .earned_runs += 1;

        let runs_cmp = self.runs_cmp();
        if runs_cmp != self.last_runs_cmp && runs_cmp != Ordering::Equal {
            // the offense took the lead; set new pitchers of record
            self.offense_mut().pitcher_of_record = *self.offense().pitchers.last().unwrap();
            self.defense_mut().pitcher_of_record = pitcher;
        }

        Ok(())
    }

    fn runs_cmp(&self) -> Ordering {
        self.game.away.runs().cmp(&self.game.home.runs())
    }

    fn walk(&mut self, event: &GameEvent) -> Result<bool> {
        if event.description.ends_with("draws a walk.") {
            self.on_base.push(Runner {
                id: self.batter()?,
                pitcher: self.pitcher(),
                base: 0,
            });
            self.fix_minimum_base();
            self.record_batter_event(|s| &mut s.plate_appearances)?;
            self.record_batter_event(|s| &mut s.walks)?;
            self.rbi_credit = self.at_bat;
            self.at_bat = None;
            self.record_pitcher_event(|s| &mut s.batters_faced)?;
            self.check_save_situation();
            self.record_pitcher_event(|s| &mut s.walks_issued)?;
            Ok(true)
        } else if let Some(name) = event.description.strip_suffix(" scores!") {
            let runner = self.name_lookup(name, event.player_tags.get(1).copied())?;
            self.credit_run(runner)?;
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
        for runner in was_on_base {
            self.credit_run(runner.id)?;
        }
        self.on_base.clear();

        Ok(true)
    }

    fn fix_minimum_base(&mut self) {
        let mut iter = self.on_base.iter_mut().rev();
        let mut last = match iter.next() {
            Some(runner) => runner.base,
            None => return,
        };
        for runner in iter {
            if runner.base <= last {
                // due to the fact that the minimum base is only used to determine RISP, and
                // because of the 🤝 glitch, we treat only first base as exclusive
                if last == 0 {
                    runner.base = 1;
                } else {
                    runner.base = last;
                }
            }
            last = runner.base;
        }
    }

    fn hit(&mut self, event: &GameEvent) -> Result<bool> {
        macro_rules! common {
            ($base:expr) => {{
                self.on_base.push(Runner {
                    id: self.batter()?,
                    pitcher: self.pitcher(),
                    base: $base,
                });
                self.fix_minimum_base();
                self.record_batter_event(|s| &mut s.plate_appearances)?;
                self.record_batter_event(|s| &mut s.at_bats)?;
                if self.risp() {
                    self.record_batter_event(|s| &mut s.at_bats_with_risp)?;
                    self.record_batter_event(|s| &mut s.hits_with_risp)?;
                }
                self.rbi_credit = self.at_bat;
                self.at_bat = None;
                self.record_pitcher_event(|s| &mut s.batters_faced)?;
                self.check_save_situation();
                self.record_pitcher_event(|s| &mut s.strikes_pitched)?;
                self.record_pitcher_event(|s| &mut s.hits_allowed)?;
                Ok(true)
            }};
        }

        let desc = &event.description;

        if event.ty == 9 && (desc.ends_with("home run!") || desc.ends_with("hits a grand slam!")) {
            self.record_batter_event(|s| &mut s.home_runs)?;
            self.record_pitcher_event(|s| &mut s.home_runs_allowed)?;
            common!(3)
        } else if event.ty == 10 && desc.ends_with("hits a Single!") {
            self.record_batter_event(|s| &mut s.singles)?;
            common!(0)
        } else if event.ty == 10 && desc.ends_with("hits a Double!") {
            self.record_batter_event(|s| &mut s.doubles)?;
            common!(1)
        } else if event.ty == 10 && desc.ends_with("hits a Triple!") {
            self.record_batter_event(|s| &mut s.triples)?;
            common!(2)
        } else if let Some(name) = desc.strip_suffix(" scores!") {
            let runner = self.name_lookup(name, event.player_tags.get(0).copied())?;
            self.credit_run(runner)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn player_trade(&mut self, a: Uuid, b: Uuid) -> Result<()> {
        self.ensure_pitchers_known()?;
        let mut a_name = self
            .game
            .teams()
            .find_map(|team| team.player_names.get(&a))
            .cloned();
        let mut b_name = self
            .game
            .teams()
            .find_map(|team| team.player_names.get(&b))
            .cloned();
        for team in self.game.teams_mut() {
            let mut insert_names = Vec::new();
            for position in team.positions_mut() {
                if position.last() == Some(&a) {
                    position.push(b);
                    if let Some(name) = b_name.take() {
                        insert_names.push((b, name));
                    }
                } else if position.last() == Some(&b) {
                    position.push(a);
                    if let Some(name) = a_name.take() {
                        insert_names.push((a, name));
                    }
                }
            }
            team.player_names.extend(insert_names);
        }
        if self.at_bat == Some(a) {
            self.at_bat = Some(b)
        } else if self.at_bat == Some(b) {
            self.at_bat = Some(a)
        }
        Ok(())
    }

    fn offense(&self) -> &Team {
        if self.top_of_inning {
            &self.game.away
        } else {
            &self.game.home
        }
    }

    fn offense_mut(&mut self) -> &mut Team {
        if self.top_of_inning {
            &mut self.game.away
        } else {
            &mut self.game.home
        }
    }

    fn defense(&self) -> &Team {
        if self.top_of_inning {
            &self.game.home
        } else {
            &self.game.away
        }
    }

    fn defense_mut(&mut self) -> &mut Team {
        if self.top_of_inning {
            &mut self.game.home
        } else {
            &mut self.game.away
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

    fn pitcher(&self) -> Uuid {
        *self.defense().pitchers.last().unwrap()
    }

    fn record_batter_event<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Stats) -> &mut u32,
    {
        let batter = self.batter()?;
        *f(self.offense_stats(batter)) += 1;
        Ok(())
    }

    fn record_runner_event<F>(&mut self, runner: Uuid, f: F) -> Result<()>
    where
        F: FnOnce(&mut Stats) -> &mut u32,
    {
        *f(self.offense_stats(runner)) += 1;
        Ok(())
    }

    fn record_pitcher_event<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Stats) -> &mut u32,
    {
        let pitcher = self.pitcher();
        *f(self.defense_stats(pitcher)) += 1;
        Ok(())
    }
}

// Reasons why a finishing pitcher _might_ be in a save situation.
#[derive(Debug, Clone, Copy, Serialize)]
enum SaveSituation {
    TyingRun,
    LeadThreeOrLess,
}

#[derive(Debug, Clone, Serialize)]
struct Runner {
    id: Uuid,
    /// pitcher to charge with earned run if this runner scores
    pitcher: Uuid,
    /// minimum base this runner is on
    base: u16,
}
