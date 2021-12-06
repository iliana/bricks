use crate::game::Stats;
use crate::table::{row, Table, TotalsTable};

pub const COLS: usize = 16;

pub fn table(iter: impl Iterator<Item = Stats>) -> TotalsTable<COLS, COLS> {
    let mut table = Table::new(
        [
            ("Earned Run Average", "ERA"),
            ("Games Played", "G"),
            ("Shutouts", "SHO"),
            ("Innings Pitched", "IP"),
            ("Hits Allowed", "H"),
            ("Runs Allowed", "R"),
            ("Home Runs Allowed", "HR"),
            ("Bases on Balls (Walks)", "BB"),
            ("Strikeouts", "SO"),
            ("Batters Faced", "BF"),
            ("Walks and Hits Per Inning Pitched", "WHIP"),
            ("Hits per 9 Innings", "H/9"),
            ("Home Runs per 9 Innings", "HR/9"),
            ("Walks per 9 Innings", "BB/9"),
            ("Strikeouts per 9 Innings", "SO/9"),
            ("Strikeout-to-Walk Ratio", "SO/BB"),
        ],
        "text-right",
    );

    let mut totals = Stats::default();
    for stats in iter {
        totals += stats;
        table.push(build_row(stats));
    }

    table.with_totals(build_row(totals))
}

fn build_row(stats: Stats) -> [String; COLS] {
    row![
        stats.earned_run_average(),
        stats.games_pitched,
        stats.shutouts,
        stats.innings_pitched(),
        stats.hits_allowed,
        stats.earned_runs,
        stats.home_runs_allowed,
        stats.walks_issued,
        stats.struck_outs,
        stats.batters_faced,
        stats.whip(),
        stats.hits_per_9(),
        stats.home_runs_per_9(),
        stats.walks_per_9(),
        stats.struck_outs_per_9(),
        stats.struck_outs_walks_ratio(),
    ]
}
