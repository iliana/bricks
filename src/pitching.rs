use crate::game::Stats;
use crate::table::{row, Table, Value};

pub const COLS: usize = 22;

pub fn table(iter: impl Iterator<Item = Stats>, league: Stats) -> Table<COLS> {
    let mut table = Table::new(
        [
            ("Wins", "W"),
            ("Losses", "L"),
            ("Win-Loss Percentage", "W-L%"),
            ("Earned Run Average", "ERA"),
            ("Games Played", "G"),
            ("Shutouts", "SHO"),
            ("Saves", "SV"),
            ("Innings Pitched", "IP"),
            ("Hits Allowed", "H"),
            ("Runs Allowed", "R"),
            ("Home Runs Allowed", "HR"),
            ("Bases on Balls (Walks)", "BB"),
            ("Strikeouts", "SO"),
            ("Batters Faced", "BF"),
            ("Adjusted ERA (100 is league average)", "ERA+"),
            ("Fielding Independent Pitching", "FIP"),
            ("Walks and Hits Per Inning Pitched", "WHIP"),
            ("Hits per 9 Innings", "H/9"),
            ("Home Runs per 9 Innings", "HR/9"),
            ("Walks per 9 Innings", "BB/9"),
            ("Strikeouts per 9 Innings", "SO/9"),
            ("Strikeout-to-Walk Ratio", "SO/BB"),
        ],
        "text-right",
        "number",
    );

    for stats in iter {
        table.push(build_row(stats, league));
    }

    table
}

pub fn build_row(stats: Stats, league: Stats) -> [Value; COLS] {
    row![
        stats.wins,
        stats.losses,
        stats.win_loss_percentage(),
        stats.earned_run_average(),
        stats.games_pitched,
        stats.shutouts,
        stats.saves,
        stats.innings_pitched(),
        stats.hits_allowed,
        stats.earned_runs,
        stats.home_runs_allowed,
        stats.walks_issued,
        stats.struck_outs,
        stats.batters_faced,
        stats.era_plus(league),
        stats.fielding_independent_pitching(league),
        stats.whip(),
        stats.hits_per_9(),
        stats.home_runs_per_9(),
        stats.walks_per_9(),
        stats.struck_outs_per_9(),
        stats.struck_outs_walks_ratio(),
    ]
}
