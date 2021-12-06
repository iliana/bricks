use crate::table::{Table, TotalsTable};
use crate::{batting, names, pitching, routes::ResponseResult, summary};
use anyhow::Result;
use askama::Template;
use rocket::get;
use rocket::response::content::Html;
use uuid::Uuid;

#[get("/team/<id>/<sim>/<season>")]
pub fn team(id: Uuid, sim: &str, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team(id, sim, season)? {
        Some(team) => Some(Html(team.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_team(id: Uuid, sim: &str, season: u16) -> Result<Option<TeamPage>> {
    let name = match names::team_name(id)? {
        Some(name) => name.name,
        None => return Ok(None),
    };

    let mut summary = summary::team_summary(id, sim, season)?.collect::<Result<Vec<_>>>()?;
    if summary.is_empty() {
        return Ok(None);
    }
    summary.sort_unstable();

    macro_rules! tabler {
        ($tabler:expr, $filter:expr) => {{
            let mut ident_table = Table::new([("Player", "")], "text-left");
            for row in summary.iter().filter($filter) {
                let player = names::player_name(row.player_id)?.unwrap_or_default();
                ident_table.push([player]);
            }
            let stats_table = $tabler(summary.iter().filter($filter).map(|row| row.stats));
            TotalsTable {
                table: stats_table.table.insert(0, ident_table),
                totals: stats_table.totals,
            }
        }};
    }

    Ok(Some(TeamPage {
        name,
        standard_batting: tabler!(batting::table, |s| !s.is_postseason && s.stats.is_batting()),
        postseason_batting: tabler!(batting::table, |s| s.is_postseason && s.stats.is_batting()),
        standard_pitching: tabler!(pitching::table, |s| !s.is_postseason
            && s.stats.is_pitching()),
        postseason_pitching: tabler!(pitching::table, |s| s.is_postseason
            && s.stats.is_pitching()),
    }))
}

#[derive(Template)]
#[template(path = "team.html")]
struct TeamPage {
    name: String,
    standard_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    postseason_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    standard_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
    postseason_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
}
