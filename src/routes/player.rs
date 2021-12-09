use crate::routes::team::rocket_uri_macro_team;
use crate::table::{Table, TotalsTable};
use crate::{batting, game::Stats, names, pitching, routes::ResponseResult, summary};
use anyhow::Result;
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
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

    let summary = summary::player_summary(id)?;
    if summary.is_empty() {
        return Ok(None);
    }

    macro_rules! tabler {
        ($tabler:ident, $filter:expr) => {{
            let mut ident_table = Table::new([("Season", ""), ("Team", "")], "text-left", "none");
            let mut stats_table = $tabler::table(std::iter::empty(), Stats::default());
            let mut totals = Stats::default();
            let mut league_totals = Stats::default();

            for row in summary.iter().filter($filter) {
                let team = names::team_name(row.team_id)?.unwrap_or_default();
                ident_table.push([format!("{:#}", row.season), team.shorthand]);
                ident_table.set_href(
                    1,
                    uri!(team(
                        id = row.team_id,
                        sim = &row.season.sim,
                        season = row.season.season
                    )),
                );

                let league = summary::league_totals(&row.season)?;
                stats_table.push($tabler::build_row(row.stats, league));
                totals += row.stats;
                league_totals += league;
            }

            TotalsTable {
                table: stats_table.insert(0, ident_table),
                totals: $tabler::build_row(totals, league_totals),
            }
        }};
    }

    let mut page = PlayerPage {
        name,
        id,
        standard_batting: tabler!(batting, |s| !s.is_postseason && s.stats.is_batting()),
        postseason_batting: tabler!(batting, |s| s.is_postseason && s.stats.is_batting()),
        standard_pitching: tabler!(pitching, |s| !s.is_postseason && s.stats.is_pitching()),
        postseason_pitching: tabler!(pitching, |s| s.is_postseason && s.stats.is_pitching()),
    };
    page.postseason_batting.table.skip("OPS+");
    page.postseason_pitching.table.skip("ERA+");
    Ok(Some(page))
}

#[derive(Template)]
#[template(path = "player.html")]
struct PlayerPage {
    name: String,
    id: Uuid,
    standard_batting: TotalsTable<{ batting::COLS + 2 }, { batting::COLS }>,
    postseason_batting: TotalsTable<{ batting::COLS + 2 }, { batting::COLS }>,
    standard_pitching: TotalsTable<{ pitching::COLS + 2 }, { pitching::COLS }>,
    postseason_pitching: TotalsTable<{ pitching::COLS + 2 }, { pitching::COLS }>,
}
