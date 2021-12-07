use crate::routes::player::rocket_uri_macro_player;
use crate::routes::team::rocket_uri_macro_team;
use crate::{batting, pitching, routes::ResponseResult, seasons::Season, summary, table::Table};
use anyhow::Result;
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
use uuid::Uuid;

#[get("/batting/<sim>/<season>")]
pub fn season_batting(sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_batting(Season { sim, season })? {
        Some(season) => Some(Html(season.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

#[get("/pitching/<sim>/<season>")]
pub fn season_pitching(sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_pitching(Season { sim, season })? {
        Some(season) => Some(Html(season.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

macro_rules! load {
    ($season:expr, $is_batting:expr, $tabler:expr, $filter:expr) => {{
        let seasons = Season::iter_recorded()?.collect::<Result<Vec<_>>>()?;
        if !seasons.iter().any(|s| s == &$season) {
            return Ok(None);
        }

        let summary = summary::season_summary(&$season)?;

        let mut ident_table = Table::new(
            [("Player", ""), ("Current Team", "Team")],
            "text-left",
            "none",
        );
        for row in summary.iter().filter($filter) {
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
        let stats_table = $tabler(summary.iter().filter($filter).map(|row| row.stats));

        Ok(Some(SeasonPage {
            table: stats_table.table.insert(0, ident_table),
            is_batting: $is_batting,
            what: if $is_batting { "Batting" } else { "Pitching" },
            season: $season,
            seasons,
        }))
    }};
}

fn load_batting(season: Season) -> Result<Option<SeasonPage<{ batting::COLS + 2 }>>> {
    load!(season, true, batting::table, |s| s.stats.is_batting())
}

fn load_pitching(season: Season) -> Result<Option<SeasonPage<{ pitching::COLS + 2 }>>> {
    load!(season, false, pitching::table, |s| s.stats.is_pitching())
}

#[derive(Template)]
#[template(path = "season.html")]
struct SeasonPage<const N: usize> {
    season: Season,
    seasons: Vec<Season>,
    is_batting: bool,
    what: &'static str,
    table: Table<N>,
}
