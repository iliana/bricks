use crate::routes::player::rocket_uri_macro_player;
use crate::routes::team::rocket_uri_macro_team;
use crate::{batting, pitching, routes::ResponseResult, seasons::Season, summary, table::Table};
use anyhow::Result;
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
use uuid::Uuid;

#[get("/batting/<sim>/<season>")]
pub fn season_player_batting(sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_player_batting(Season { sim, season })? {
        Some(season) => Some(Html(season.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

#[get("/pitching/<sim>/<season>")]
pub fn season_player_pitching(sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_player_pitching(Season { sim, season })? {
        Some(season) => Some(Html(season.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

#[get("/batting/team/<sim>/<season>")]
pub fn season_team_batting(sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team_batting(Season { sim, season })? {
        Some(season) => Some(Html(season.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

#[get("/pitching/team/<sim>/<season>")]
pub fn season_team_pitching(sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team_pitching(Season { sim, season })? {
        Some(season) => Some(Html(season.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

macro_rules! load {
    ($season:expr, $summary_func:ident, $is_batting:expr, $tabler:expr, $filter:expr) => {{
        let seasons = Season::recorded()?;
        if !seasons.iter().any(|s| s == &$season) {
            return Ok(None);
        }

        let summary = summary::$summary_func(&$season)?;
        let league = summary::league_totals(&$season)?;
        let stats_table = $tabler(summary.iter().filter($filter).map(|row| row.stats), league);

        Ok(Some(SeasonPage {
            table: load!(@inner $summary_func, summary, stats_table, $season, $filter),
            is_players: stringify!($summary_func) == "season_player_summary",
            is_batting: $is_batting,
            what: if $is_batting { "Batting" } else { "Pitching" },
            season: $season,
            seasons,
        }))
    }};

    (@inner season_player_summary, $summary:expr, $table:expr, $season:expr, $filter:expr) => {{
        let mut ident_table = Table::new(
            [("Player", ""), ("Current Team", "Team")],
            "text-left",
            "none",
        );
        for row in $summary.iter().filter($filter) {
            ident_table.push([row.name.clone(), row.team_abbr.clone()]);
            ident_table.set_href(0, uri!(player(id = row.id)));
            ident_table.set_href(
                1,
                uri!(team(
                    id = row.team_id,
                    sim = &$season.sim,
                    season = $season.season
                )),
            );
        }
        $table.insert(0, ident_table)
    }};

    (@inner season_team_summary, $summary:expr, $table:expr, $season:expr, $filter:expr) => {{
        let mut ident_table = Table::new([("Team", "")], "text-left", "none");
        for row in $summary.iter().filter($filter) {
            ident_table.push([row.name.clone()]);
            ident_table.set_href(
                0,
                uri!(team(
                    id = row.id,
                    sim = &$season.sim,
                    season = $season.season
                )),
            );
        }
        $table.insert(0, ident_table)
    }};
}

fn load_player_batting(season: Season) -> Result<Option<SeasonPage<{ batting::COLS + 2 }>>> {
    load!(season, season_player_summary, true, batting::table, |s| s
        .stats
        .is_batting())
}

fn load_player_pitching(season: Season) -> Result<Option<SeasonPage<{ pitching::COLS + 2 }>>> {
    load!(season, season_player_summary, false, pitching::table, |s| s
        .stats
        .is_pitching())
}

fn load_team_batting(season: Season) -> Result<Option<SeasonPage<{ batting::COLS + 1 }>>> {
    load!(season, season_team_summary, true, batting::table, |s| s
        .stats
        .is_batting())
}

fn load_team_pitching(season: Season) -> Result<Option<SeasonPage<{ pitching::COLS + 1 }>>> {
    load!(season, season_team_summary, false, pitching::table, |s| s
        .stats
        .is_pitching())
}

#[derive(Template)]
#[template(path = "season.html")]
struct SeasonPage<const N: usize> {
    season: Season,
    seasons: Vec<Season>,
    is_players: bool,
    is_batting: bool,
    what: &'static str,
    table: Table<N>,
}
