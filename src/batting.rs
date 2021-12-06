use crate::game::Stats;
use crate::table::{row, Table, TotalsTable};

pub const COLS: usize = 21;

pub fn table(iter: impl Iterator<Item = Stats>) -> TotalsTable<COLS, COLS> {
    let mut table = Table::new(
        [
            ("Games Played", "G"),
            ("Plate Appearances", "PA"),
            ("At Bats", "AB"),
            ("Runs Scored", "R"),
            ("Hits", "H"),
            ("Doubles", "2B"),
            ("Triples", "3B"),
            ("Home Runs", "HR"),
            ("Runs Batted In", "RBI"),
            ("Stolen Bases", "SB"),
            ("Caught Stealing", "CS"),
            ("Bases on Balls (Walks)", "BB"),
            ("Strikeouts", "SO"),
            ("Batting Average", "BA"),
            ("On-base Percentage", "OBP"),
            ("Slugging Percentage", "SLG"),
            ("On-base Plus Slugging", "OPS"),
            ("Total Bases", "TB"),
            ("Double Plays Grounded Into", "GIDP"),
            ("Sacrifice Hits", "SH"),
            ("Sacrifice Flies", "SF"),
        ],
        "text-right",
        "number",
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
        stats.games_batted,
        stats.plate_appearances,
        stats.at_bats,
        stats.runs,
        stats.hits(),
        stats.doubles,
        stats.triples,
        stats.home_runs,
        stats.runs_batted_in,
        stats.stolen_bases,
        stats.caught_stealing,
        stats.walks,
        stats.strike_outs,
        stats.batting_average(),
        stats.on_base_percentage(),
        stats.slugging_percentage(),
        stats.on_base_plus_slugging(),
        stats.total_bases(),
        stats.double_plays_grounded_into,
        stats.sacrifice_hits,
        stats.sacrifice_flies,
    ]
}
