use derive_more::{Add, Sum};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::iter::{Chain, Once};
use uuid::Uuid;

fn box_list<'a, F, S>(
    iter: impl IntoIterator<Item = &'a GameStats>,
    f: F,
    force_number: bool,
) -> String
where
    F: Fn(&Stats) -> S,
    S: ToString,
{
    let mut v = Vec::new();
    for game_stats in iter {
        for (id, stats) in &game_stats.stats {
            let stat = f(stats).to_string();
            if stat.is_empty() || stat == "0" {
                // nothing
            } else if stat == "1" && !force_number {
                v.push(game_stats.box_name(id).into());
            } else {
                v.push(format!("{}\u{a0}{}", game_stats.box_name(id), stat));
            }
        }
    }
    v.join("; ")
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct AwayHome<T> {
    pub(crate) away: T,
    pub(crate) home: T,
}

impl<T> AwayHome<T> {
    pub(crate) fn teams_mut(&mut self) -> impl Iterator<Item = &mut T> {
        [&mut self.away, &mut self.home].into_iter()
    }
}

impl<'a, T> IntoIterator for &'a AwayHome<T> {
    type Item = &'a T;
    type IntoIter = Chain<Once<&'a T>, Once<&'a T>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.away).chain(std::iter::once(&self.home))
    }
}

impl AwayHome<GameStats> {
    pub(crate) fn box_pitching_lists(&self) -> BoxList {
        let mut lists = vec![
            (
                "",
                "Pitches-strikes",
                box_list([&self.away, &self.home], |s| s.pitches_strikes(), false),
            ),
            (
                "",
                "Groundouts-flyouts",
                box_list([&self.away, &self.home], |s| s.groundouts_flyouts(), false),
            ),
            (
                "",
                "Batters faced",
                box_list([&self.away, &self.home], |s| s.batters_faced, true),
            ),
        ];
        lists.retain(|(_, _, s)| !s.is_empty());
        lists
    }
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct GameStats {
    pub(crate) team: Uuid,
    pub(crate) name: String,
    pub(crate) nickname: String,
    pub(crate) shorthand: String,
    pub(crate) emoji: String,
    pub(crate) player_names: HashMap<Uuid, String>,
    pub(crate) player_box_names: HashMap<Uuid, String>,
    pub(crate) lineup: Vec<Vec<Uuid>>,
    pub(crate) pitchers: Vec<Uuid>,

    pub(crate) stats: IndexMap<Uuid, Stats>,
    pub(crate) totals: Stats,
    pub(crate) inning_run_totals: BTreeMap<u16, u16>,
    pub(crate) left_on_base: usize,
}

type BoxList = Vec<(&'static str, &'static str, String)>;

impl GameStats {
    pub(crate) fn box_names(&mut self) {
        let mut last_names: HashMap<&str, usize> = HashMap::new();
        for name in self.player_names.values() {
            *last_names.entry(name).or_default() += 1;
        }

        self.player_box_names = self
            .player_names
            .iter()
            .map(|(id, name)| {
                let mut iter = name.rsplitn(2, ' ');
                let last = iter.next().unwrap();
                let rem = iter.next();

                let box_name = if last_names.get(last).copied().unwrap_or_default() > 1 {
                    if let Some(rem) = rem {
                        let first = rem
                            .split(' ')
                            .filter_map(|s| s.chars().next().map(String::from))
                            .collect::<Vec<_>>();
                        format!("{}, {}", last, first.join(" "))
                    } else {
                        last.into()
                    }
                } else {
                    last.into()
                };

                (*id, box_name)
            })
            .collect();
    }

    pub(crate) fn runs(&self) -> u16 {
        self.inning_run_totals.values().sum()
    }

    pub(crate) fn box_name(&self, id: &Uuid) -> &str {
        self.player_box_names
            .get(id)
            .map(String::as_str)
            .unwrap_or_default()
    }

    pub(crate) fn player_stats(&self, id: &Uuid) -> Stats {
        self.stats.get(id).copied().unwrap_or_default()
    }

    fn box_list<F>(&self, f: F, force_number: bool) -> String
    where
        F: Fn(&Stats) -> u16,
    {
        box_list([self], f, force_number)
    }

    pub(crate) fn box_batting_lists(&self) -> BoxList {
        let mut lists = vec![
            ("2B", "Doubles", self.box_list(|s| s.doubles, false)),
            ("3B", "Triples", self.box_list(|s| s.triples, false)),
            ("HR", "Home Runs", self.box_list(|s| s.home_runs, false)),
            (
                "TB",
                "Total Bases",
                self.box_list(|s| s.total_bases(), true),
            ),
            (
                "GDP",
                "Double Plays Grounded Into",
                self.box_list(|s| s.double_plays_grounded_into, false),
            ),
        ];
        lists.retain(|(_, _, s)| !s.is_empty());
        if self.totals.at_bats_with_risp > 0 {
            lists.push((
                "Team RISP",
                "Team Hits with Runners in Scoring Position",
                format!(
                    "{}-for-{}",
                    self.totals.hits_with_risp, self.totals.at_bats_with_risp
                ),
            ));
        }
        if self.left_on_base > 0 {
            lists.push((
                "Team LOB",
                "Team Runners Left on Bases",
                self.left_on_base.to_string(),
            ));
        }
        lists
    }

    pub(crate) fn box_baserunning_lists(&self) -> BoxList {
        let mut lists = vec![
            (
                "SB",
                "Stolen Bases",
                self.box_list(|s| s.stolen_bases, false),
            ),
            (
                "CS",
                "Caught Stealing",
                self.box_list(|s| s.caught_stealing, false),
            ),
        ];
        lists.retain(|(_, _, s)| !s.is_empty());
        lists
    }
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, Add, Sum)]
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
    pub(crate) runs_batted_in: u16,
    pub(crate) sacrifice_hits: u16,
    pub(crate) sacrifice_flies: u16,
    pub(crate) stolen_bases: u16,
    pub(crate) caught_stealing: u16,
    pub(crate) strike_outs: u16,
    pub(crate) double_plays_grounded_into: u16,
    pub(crate) walks: u16,
    pub(crate) left_on_base: usize,

    // Pitching stats
    pub(crate) batters_faced: u16,
    pub(crate) outs_recorded: u16,
    pub(crate) hits_allowed: u16,
    pub(crate) home_runs_allowed: u16,
    pub(crate) earned_runs: u16,
    pub(crate) struck_outs: u16,
    pub(crate) walks_issued: u16,
    pub(crate) strikes_pitched: u16,
    pub(crate) balls_pitched: u16,
    pub(crate) flyouts_pitched: u16,
    pub(crate) groundouts_pitched: u16,
}

impl Stats {
    pub(crate) fn hits(&self) -> u16 {
        self.singles + self.doubles + self.triples + self.home_runs
    }

    pub(crate) fn total_bases(&self) -> u16 {
        self.singles + 2 * self.doubles + 3 * self.triples + 4 * self.home_runs
    }

    pub(crate) fn innings_pitched(&self) -> String {
        format!("{}.{}", self.outs_recorded / 3, self.outs_recorded % 3)
    }

    pub(crate) fn pitches_strikes(&self) -> String {
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

    pub(crate) fn groundouts_flyouts(&self) -> String {
        if self.groundouts_pitched + self.flyouts_pitched > 0 {
            format!("{}-{}", self.groundouts_pitched, self.flyouts_pitched)
        } else {
            String::new()
        }
    }
}
