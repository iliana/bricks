use crate::names::{self, TeamName};
use crate::routes::player::rocket_uri_macro_player;
use crate::schedule::{self, Entry, Record};
use crate::table::{Table, TotalsTable};
use crate::{batting, pitching, routes::ResponseResult, seasons::Season, summary};
use anyhow::Result;
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
use uuid::Uuid;

#[get("/team/<id>/<sim>/<season>")]
pub fn team(id: Uuid, sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team(id, Season { sim, season })? {
        Some(team) => Some(Html(team.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_team(id: Uuid, season: Season) -> Result<Option<TeamPage>> {
    let name = match names::team_name(id)? {
        Some(name) => name,
        None => return Ok(None),
    };

    let seasons = name.all_seasons()?;
    if !seasons.iter().any(|(s, _)| s == &season) {
        return Ok(None);
    }

    let schedule = schedule::schedule(id, &season)?;
    let ceiling = schedule
        .iter()
        .map(|(r, _)| r.diff())
        .max()
        .unwrap_or_default()
        .max(0);
    let floor = schedule
        .iter()
        .map(|(r, _)| r.diff())
        .min()
        .unwrap_or_default()
        .min(0);

    let summary = summary::team_summary(id, &season)?;

    macro_rules! tabler {
        ($tabler:ident, $is_postseason:expr, $filter:expr) => {{
            let mut ident_table = Table::new([("Player", "")], "text-left", "none");
            for row in summary.iter().filter($filter) {
                let player = names::player_name(row.player_id)?.unwrap_or_default();
                ident_table.push([player]);
                ident_table.set_href(0, uri!(player(id = row.player_id)));
            }
            let stats_table = $tabler::table(summary.iter().filter($filter).map(|row| row.stats));
            let totals = summary::team_totals(&season, id, $is_postseason)?
                .map($tabler::build_row)
                .unwrap_or(stats_table.totals);
            TotalsTable {
                table: stats_table.table.insert(0, ident_table),
                totals,
            }
        }};
    }

    Ok(Some(TeamPage {
        team: name,
        seasons,
        schedule,
        ceiling,
        floor,
        standard_batting: tabler!(batting, false, |s| !s.is_postseason && s.stats.is_batting()),
        postseason_batting: tabler!(batting, true, |s| s.is_postseason && s.stats.is_batting()),
        standard_pitching: tabler!(pitching, false, |s| !s.is_postseason
            && s.stats.is_pitching()),
        postseason_pitching: tabler!(pitching, true, |s| s.is_postseason && s.stats.is_pitching()),
        season,
    }))
}

#[derive(Template)]
#[template(path = "team.html")]
struct TeamPage {
    team: TeamName,
    season: Season,
    seasons: Vec<(Season, Uuid)>,
    schedule: Vec<(Record, Entry)>,
    ceiling: i32,
    floor: i32,
    standard_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    postseason_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    standard_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
    postseason_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
}
