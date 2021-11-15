use crate::summary::{self, Summary};
use crate::table::{row, Table, TotalsTable};
use crate::{game::Stats, names, routes::ResponseResult};
use anyhow::Result;
use askama::Template;
use rocket::get;
use rocket::response::content::Html;
use uuid::Uuid;

#[get("/player/<id>")]
pub fn player(id: Uuid) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_player(id)? {
        Some(player) => Some(Html(player.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_player(id: Uuid) -> Result<Option<PlayerPage>> {
    let name = match names::player_name(id)? {
        Some(name) => name,
        None => return Ok(None),
    };

    let summary = summary::player_summary(id)?.collect::<Result<Vec<_>>>()?;
    if summary.is_empty() {
        return Ok(None);
    }

    Ok(Some(PlayerPage {
        name,
        standard_batting: batting_table(&summary, false)?,
        postseason_batting: batting_table(&summary, true)?,
        standard_pitching: pitching_table(&summary, false)?,
        postseason_pitching: pitching_table(&summary, true)?,
    }))
}

#[derive(Template)]
#[template(path = "player.html")]
struct PlayerPage {
    name: String,
    standard_batting: TotalsTable<BATTING_COLS, { BATTING_COLS - BATTING_SKIP_COLS }>,
    postseason_batting: TotalsTable<BATTING_COLS, { BATTING_COLS - BATTING_SKIP_COLS }>,
    standard_pitching: TotalsTable<PITCHING_COLS, { PITCHING_COLS - PITCHING_SKIP_COLS }>,
    postseason_pitching: TotalsTable<PITCHING_COLS, { PITCHING_COLS - PITCHING_SKIP_COLS }>,
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

const BATTING_COLS: usize = 23;
const BATTING_SKIP_COLS: usize = 2;

fn batting_table(
    summary: &[Summary],
    is_postseason: bool,
) -> Result<TotalsTable<BATTING_COLS, { BATTING_COLS - BATTING_SKIP_COLS }>> {
    fn build_row(stats: &Stats) -> [String; BATTING_COLS - BATTING_SKIP_COLS] {
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

    let mut table = Table::new(
        [
            ("Season", ""),
            ("Team", ""),
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
    );
    table.col_class[0] = "text-left";
    table.col_class[1] = "text-left";

    let mut totals = Stats::default();
    for row in summary {
        if row.is_postseason != is_postseason || !row.stats.is_batting() {
            continue;
        }

        totals += row.stats;

        let mut stats: [String; BATTING_COLS] = Default::default();
        stats[0] = format!("{}/S{}", row.era, row.season + 1);
        let team = names::team_name(row.team_id)?.unwrap_or_default();
        stats[1] = format!("{} {}", team.emoji, team.shorthand);
        for (i, s) in build_row(&row.stats).into_iter().enumerate() {
            stats[i + BATTING_SKIP_COLS] = s;
        }
        table.rows.push((stats, ""));
    }

    Ok(table.with_totals(build_row(&totals)))
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

const PITCHING_COLS: usize = 17;
const PITCHING_SKIP_COLS: usize = 2;

fn pitching_table(
    summary: &[Summary],
    is_postseason: bool,
) -> Result<TotalsTable<PITCHING_COLS, { PITCHING_COLS - PITCHING_SKIP_COLS }>> {
    fn build_row(stats: &Stats) -> [String; PITCHING_COLS - PITCHING_SKIP_COLS] {
        row![
            stats.earned_run_average(),
            stats.games_pitched,
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

    let mut table = Table::new(
        [
            ("Season", ""),
            ("Team", ""),
            ("Earned Run Average", "ERA"),
            ("Games Played", "G"),
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
    table.col_class[0] = "text-left";
    table.col_class[1] = "text-left";

    let mut totals = Stats::default();
    for row in summary {
        if row.is_postseason != is_postseason || !row.stats.is_pitching() {
            continue;
        }

        totals += row.stats;

        let mut stats: [String; PITCHING_COLS] = Default::default();
        stats[0] = format!("{}/S{}", row.era, row.season + 1);
        let team = names::team_name(row.team_id)?.unwrap_or_default();
        stats[1] = format!("{} {}", team.emoji, team.shorthand);
        for (i, s) in build_row(&row.stats).into_iter().enumerate() {
            stats[i + PITCHING_SKIP_COLS] = s;
        }
        table.rows.push((stats, ""));
    }

    Ok(table.with_totals(build_row(&totals)))
}
